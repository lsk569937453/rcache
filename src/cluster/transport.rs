use crate::parser::request::Request;
use crate::parser::response::Response;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Build a RESP array message from a list of byte slices.
///
/// Example: `build_cluster_message(&[b"PING", b"node_id"])` produces
/// `*2\r\n$4\r\nPING\r\n$7\r\nnode_id\r\n`
pub fn build_cluster_message(parts: &[&[u8]]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(format!("*{}\r\n", parts.len()).as_bytes());
    for part in parts {
        buf.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        buf.extend_from_slice(part);
        buf.extend_from_slice(b"\r\n");
    }
    buf
}

/// Send a RESP-encoded cluster message to a target node and read the response.
pub async fn send_cluster_message(
    target_addr: &str,
    message: &[u8],
) -> Result<Vec<u8>, anyhow::Error> {
    let mut stream = TcpStream::connect(target_addr).await?;
    stream.write_all(message).await?;

    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

/// Parse a cluster response (RESP-encoded) and extract the raw data.
pub fn parse_cluster_response(data: &[u8]) -> Result<Vec<Response>, anyhow::Error> {
    let commands = Request::parse_all(data)?;
    // We treat each parsed command as a container of arguments,
    // and reconstruct simple responses from them.
    // For gossip, we actually want the raw bytes, so just return success.
    let _ = commands;
    Ok(vec![])
}

/// Send a MEET message to a target node.
pub async fn send_meet(
    target_addr: &str,
    sender_id: &str,
    sender_client_addr: &str,
) -> Result<(), anyhow::Error> {
    let msg = build_cluster_message(&[
        b"MEET",
        sender_id.as_bytes(),
        sender_client_addr.as_bytes(),
    ]);
    let _response = send_cluster_message(target_addr, &msg).await?;
    Ok(())
}

/// Send a PING message with cluster state digest.
pub async fn send_ping(
    target_addr: &str,
    sender_id: &str,
    state_digest: &[u8],
) -> Result<Vec<u8>, anyhow::Error> {
    let msg = build_cluster_message(&[b"PING", sender_id.as_bytes(), state_digest]);
    send_cluster_message(target_addr, &msg).await
}

/// Build a gossip digest JSON string representing the cluster state.
/// Format: JSON array of [node_id, ip:port, flags_json, slots_json, epoch] per node.
pub fn build_state_digest(state: &crate::cluster::state::ClusterState) -> Vec<u8> {
    let mut entries = Vec::new();

    for node in state.nodes.values() {
        let slots_str: Vec<String> = node
            .slot_ranges
            .iter()
            .map(|r| {
                if r.start == r.end {
                    format!("{}", r.start)
                } else {
                    format!("{}-{}", r.start, r.end)
                }
            })
            .collect();

        let flags_str: Vec<String> = node.flags.iter().map(|f| format!("{}", f)).collect();

        let entry = serde_json::json!({
            "id": node.id,
            "addr": node.addr.to_string(),
            "flags": flags_str,
            "slots": slots_str,
            "epoch": node.config_epoch,
        });
        entries.push(entry);
    }

    let digest = serde_json::json!({
        "sender_id": state.myself.id,
        "sender_addr": state.myself.addr.to_string(),
        "config_epoch": state.config_epoch,
        "nodes": entries,
    });

    serde_json::to_vec(&digest).unwrap_or_default()
}

/// Parse a gossip digest JSON into its components.
pub fn parse_state_digest(
    data: &[u8],
) -> Result<
    (
        String,
        std::net::SocketAddr,
        u64,
        Vec<(String, std::net::SocketAddr, Vec<crate::cluster::node::NodeFlags>, Vec<crate::cluster::node::SlotRange>, u64)>,
    ),
    anyhow::Error,
> {
    let digest: serde_json::Value = serde_json::from_slice(data)?;

    let sender_id = digest["sender_id"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let sender_addr_str = digest["sender_addr"]
        .as_str()
        .unwrap_or("0.0.0.0:0");
    let sender_addr: std::net::SocketAddr = sender_addr_str.parse()?;
    let config_epoch = digest["config_epoch"]
        .as_u64()
        .unwrap_or(0);

    let mut node_digests = Vec::new();

    if let Some(nodes) = digest["nodes"].as_array() {
        for node_entry in nodes {
            let node_id = node_entry["id"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let node_addr_str = node_entry["addr"]
                .as_str()
                .unwrap_or("0.0.0.0:0");
            let node_addr: std::net::SocketAddr = match node_addr_str.parse() {
                Ok(a) => a,
                Err(_) => continue,
            };

            let flags = parse_flags(&node_entry["flags"]);
            let slot_ranges = parse_slot_ranges(&node_entry["slots"]);
            let epoch = node_entry["epoch"].as_u64().unwrap_or(0);

            node_digests.push((node_id, node_addr, flags, slot_ranges, epoch));
        }
    }

    Ok((sender_id, sender_addr, config_epoch, node_digests))
}

fn parse_flags(flags_val: &serde_json::Value) -> Vec<crate::cluster::node::NodeFlags> {
    use crate::cluster::node::NodeFlags;

    let mut result = Vec::new();
    if let Some(arr) = flags_val.as_array() {
        for flag_str in arr {
            if let Some(s) = flag_str.as_str() {
                match s {
                    "myself" => result.push(NodeFlags::Myself),
                    "master" => result.push(NodeFlags::Master),
                    "slave" => result.push(NodeFlags::Slave),
                    "handshake" => result.push(NodeFlags::Handshake),
                    "noaddr" => result.push(NodeFlags::NoAddr),
                    _ => {}
                }
            }
        }
    }
    result
}

fn parse_slot_ranges(slots_val: &serde_json::Value) -> Vec<crate::cluster::node::SlotRange> {
    use crate::cluster::node::SlotRange;

    let mut result = Vec::new();
    if let Some(arr) = slots_val.as_array() {
        for slot_str in arr {
            if let Some(s) = slot_str.as_str() {
                if let Some(dash_pos) = s.find('-') {
                    if let (Ok(start), Ok(end)) =
                        (s[..dash_pos].parse::<u16>(), s[dash_pos + 1..].parse::<u16>())
                    {
                        result.push(SlotRange { start, end });
                    }
                } else if let Ok(slot) = s.parse::<u16>() {
                    result.push(SlotRange {
                        start: slot,
                        end: slot,
                    });
                }
            }
        }
    }
    result
}
