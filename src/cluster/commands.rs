use crate::cluster::gossip;
use crate::cluster::node::{ClusterNode, SlotRange};
use crate::cluster::slot::{key_hash_slot, CLUSTER_HASH_SLOTS};
use crate::cluster::state::ClusterHolder;
use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;
use anyhow::anyhow;

/// Main CLUSTER command dispatcher.
pub async fn cluster_command(
    cmd: ParsedCommand,
    cluster_holder: &ClusterHolder,
    database_holder: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 2 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster' command"
        ));
    }

    let subcommand = cmd.get_str(1)?.to_uppercase();

    match subcommand.as_str() {
        "MEET" => cluster_meet(cmd, cluster_holder).await,
        "INFO" => cluster_info(cluster_holder).await,
        "NODES" => cluster_nodes(cluster_holder).await,
        "SLOTS" => cluster_slots(cluster_holder).await,
        "MYID" => cluster_myid(cluster_holder).await,
        "ADDSLOTS" => cluster_addslots(cmd, cluster_holder).await,
        "DELSLOTS" => cluster_delslots(cmd, cluster_holder).await,
        "FLUSHSLOTS" => cluster_flushslots(cluster_holder).await,
        "KEYSLOT" => cluster_keyslot(cmd),
        "RESET" => cluster_reset(cluster_holder).await,
        "SET-CONFIG-EPOCH" => cluster_set_config_epoch(cmd, cluster_holder).await,
        "REPLICATE" => cluster_replicate(cmd, cluster_holder).await,
        "COUNTKEYSINSLOT" => cluster_count_keys_in_slot(cmd, database_holder, db_index),
        _ => Err(anyhow!(
            "ERR Unknown CLUSTER subcommand '{}'",
            subcommand
        )),
    }
}

/// CLUSTER MEET ip port
/// Add a node to the cluster and initiate a gossip handshake.
async fn cluster_meet(
    cmd: ParsedCommand,
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 4 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|meet' command"
        ));
    }

    let ip = cmd.get_str(2)?;
    let port: u16 = cmd.get_str(3)?.parse()?;

    let node_addr: std::net::SocketAddr = format!("{}:{}", ip, port).parse()?;

    // Add the node to our cluster state
    {
        let mut state = cluster_holder.state.write().await;

        // Check if already known
        let already_known = state
            .nodes
            .values()
            .any(|n| n.addr == node_addr);

        if !already_known {
            // Generate a temporary node ID for the new node
            let node_id = crate::cluster::node::generate_node_id(
                &node_addr,
                chrono::Utc::now().timestamp(),
            );
            let new_node = ClusterNode::new(node_id, node_addr);
            state.add_node(new_node);
            info!("CLUSTER MEET: added node at {}", node_addr);
        }
    }

    // Send MEET message to the target node's bus port
    let bus_port = port + 10000;
    let bus_addr = format!("{}:{}", ip, bus_port);

    let sender_id = {
        let state = cluster_holder.state.read().await;
        state.myself.id.clone()
    };
    let sender_client_addr = {
        let state = cluster_holder.state.read().await;
        state.myself.addr.to_string()
    };

    // Fire-and-forget MEET message (don't block on it)
    match gossip::send_meet_direct(&bus_addr, &sender_id, &sender_client_addr).await {
        Ok(()) => info!("CLUSTER MEET: sent MEET to {}", bus_addr),
        Err(e) => info!("CLUSTER MEET: could not reach {} (will be discovered via gossip): {}", bus_addr, e),
    }

    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER INFO
/// Return cluster state information.
async fn cluster_info(cluster_holder: &ClusterHolder) -> Result<Response, anyhow::Error> {
    let state = cluster_holder.state.read().await;

    let assigned = state.assigned_slot_count();
    let known_nodes = state.nodes.len();
    let master_count = state.master_count();
    let my_epoch = state.myself.config_epoch;

    let info = format!(
        "cluster_state:{}\r\n\
         cluster_slots_assigned:{}\r\n\
         cluster_slots_ok:{}\r\n\
         cluster_slots_pfail:0\r\n\
         cluster_slots_fail:0\r\n\
         cluster_known_nodes:{}\r\n\
         cluster_size:{}\r\n\
         cluster_current_epoch:{}\r\n\
         cluster_my_epoch:{}\r\n\
         cluster_stats_messages_sent:0\r\n\
         cluster_stats_messages_received:0",
        state.health,
        assigned,
        assigned,
        known_nodes,
        master_count,
        state.config_epoch,
        my_epoch,
    );

    Ok(Response::Data(info.into_bytes()))
}

/// CLUSTER NODES
/// Return all known nodes in Redis cluster nodes format.
async fn cluster_nodes(cluster_holder: &ClusterHolder) -> Result<Response, anyhow::Error> {
    let state = cluster_holder.state.read().await;
    let bus_port = state.myself.bus_port();

    let mut lines = Vec::new();
    for node in state.nodes.values() {
        lines.push(node.to_cluster_nodes_line(bus_port));
    }

    let result = lines.join("\n");
    Ok(Response::Data(result.into_bytes()))
}

/// CLUSTER SLOTS
/// Return slot-to-node mapping.
async fn cluster_slots(cluster_holder: &ClusterHolder) -> Result<Response, anyhow::Error> {
    let state = cluster_holder.state.read().await;
    let slot_info = state.cluster_slots_info();

    let mut entries = Vec::new();
    for (start, end, nodes) in slot_info {
        let mut entry = vec![
            Response::Integer(start as i64),
            Response::Integer(end as i64),
        ];
        for (id, ip, port) in nodes {
            entry.push(Response::Array(vec![
                Response::Data(ip.into_bytes()),
                Response::Integer(port as i64),
                Response::Data(id.into_bytes()),
            ]));
        }
        entries.push(Response::Array(entry));
    }

    Ok(Response::Array(entries))
}

/// CLUSTER MYID
/// Return this node's ID.
async fn cluster_myid(cluster_holder: &ClusterHolder) -> Result<Response, anyhow::Error> {
    let state = cluster_holder.state.read().await;
    Ok(Response::Data(state.myself.id.as_bytes().to_vec()))
}

/// CLUSTER ADDSLOTS slot [slot ...]
/// Assign hash slots to this node.
async fn cluster_addslots(
    cmd: ParsedCommand,
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|addslots' command"
        ));
    }

    let mut slots = Vec::new();
    for i in 2..cmd.argv.len() {
        let arg = cmd.get_str(i)?;
        // Support range format: "0-5460"
        if let Some(dash_pos) = arg.find('-') {
            let start: u16 = arg[..dash_pos].parse()?;
            let end: u16 = arg[dash_pos + 1..].parse()?;
            for s in start..=end {
                slots.push(s);
            }
        } else {
            let slot: u16 = arg.parse()?;
            slots.push(slot);
        }
    }

    // Validate slots
    for &slot in &slots {
        if slot >= CLUSTER_HASH_SLOTS {
            return Err(anyhow!("ERR Slot {} is out of range", slot));
        }
    }

    // Group consecutive slots into ranges
    let ranges = group_slots_into_ranges(&slots);

    let my_id = {
        let state = cluster_holder.state.read().await;
        state.myself.id.clone()
    };

    {
        let mut state = cluster_holder.state.write().await;
        state.add_slots(&ranges, &my_id);
    }

    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER DELSLOTS slot [slot ...]
/// Remove slot assignments.
async fn cluster_delslots(
    cmd: ParsedCommand,
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|delslots' command"
        ));
    }

    let mut slots = Vec::new();
    for i in 2..cmd.argv.len() {
        let slot: u16 = cmd.get_str(i)?.parse()?;
        if slot >= CLUSTER_HASH_SLOTS {
            return Err(anyhow!("ERR Slot {} is out of range", slot));
        }
        slots.push(slot);
    }

    {
        let mut state = cluster_holder.state.write().await;
        state.del_slots(&slots);
    }

    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER FLUSHSLOTS
/// Remove all slot assignments.
async fn cluster_flushslots(
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    let mut state = cluster_holder.state.write().await;
    state.flush_slots();
    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER KEYSLOT key
/// Show the hash slot for a key.
fn cluster_keyslot(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|keyslot' command"
        ));
    }
    let key = cmd.get_slice(2)?;
    let slot = key_hash_slot(key);
    Ok(Response::Integer(slot as i64))
}

/// CLUSTER RESET [HARD|SOFT]
/// Reset cluster state.
async fn cluster_reset(
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    let mut state = cluster_holder.state.write().await;
    state.reset();
    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER SET-CONFIG-EPOCH epoch
async fn cluster_set_config_epoch(
    cmd: ParsedCommand,
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|set-config-epoch' command"
        ));
    }
    let epoch: u64 = cmd.get_str(2)?.parse()?;

    let mut state = cluster_holder.state.write().await;
    state.config_epoch = epoch;
    state.myself.config_epoch = epoch;

    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER REPLICATE node_id
/// Become a slave of the specified master node.
async fn cluster_replicate(
    cmd: ParsedCommand,
    cluster_holder: &ClusterHolder,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|replicate' command"
        ));
    }

    let master_id = cmd.get_str(2)?.to_string();

    let mut state = cluster_holder.state.write().await;

    // Verify master exists
    if !state.nodes.contains_key(&master_id) {
        return Err(anyhow!("ERR Unknown node {}", master_id));
    }

    // Remove myself from master role
    state.myself.flags.retain(|f| f != &crate::cluster::node::NodeFlags::Master);
    state
        .myself
        .flags
        .push(crate::cluster::node::NodeFlags::Slave);
    state.myself.master_id = Some(master_id.clone());
    state.myself.slot_ranges.clear();

    // Clear my slots from the slot map (clone my_id to avoid borrow conflict)
    let my_id = state.myself.id.clone();
    for slot in state.slots.iter_mut() {
        if slot.as_deref() == Some(&my_id) {
            *slot = None;
        }
    }

    // Update in nodes map too
    let my_flags = state.myself.flags.clone();
    if let Some(my_node) = state.nodes.get_mut(&my_id) {
        my_node.flags = my_flags;
        my_node.master_id = Some(master_id);
        my_node.slot_ranges.clear();
    }

    state.update_health();

    Ok(Response::Status("OK".to_string()))
}

/// CLUSTER COUNTKEYSINSLOT slot
/// Count the number of keys in the given slot on this node.
fn cluster_count_keys_in_slot(
    cmd: ParsedCommand,
    database_holder: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 3 {
        return Err(anyhow!(
            "ERR wrong number of arguments for 'cluster|countkeysinslot' command"
        ));
    }

    let slot: u16 = cmd.get_str(2)?.parse()?;
    if slot >= CLUSTER_HASH_SLOTS {
        return Err(anyhow!("ERR Slot {} is out of range", slot));
    }

    // Count keys that hash to this slot
    let db = database_holder.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let data = db.data.get(db_index);
    let count = match data {
        Some(map) => map
            .keys()
            .filter(|k| key_hash_slot(k) == slot)
            .count(),
        None => 0,
    };

    Ok(Response::Integer(count as i64))
}

/// Group a list of slot numbers into consecutive ranges.
fn group_slots_into_ranges(slots: &[u16]) -> Vec<SlotRange> {
    if slots.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<u16> = slots.to_vec();
    sorted.sort();
    sorted.dedup();

    let mut ranges = Vec::new();
    let mut start = sorted[0];
    let mut end = sorted[0];

    for &slot in &sorted[1..] {
        if slot == end + 1 {
            end = slot;
        } else {
            ranges.push(SlotRange { start, end });
            start = slot;
            end = slot;
        }
    }
    ranges.push(SlotRange { start, end });

    ranges
}
