use crate::cluster::state::ClusterHolder;
use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;
use anyhow::anyhow;

/// HELLO [protover [AUTH username password] [SETNAME clientname]]
/// Switch to RESP3 and optionally authenticate
///
/// This is a simplified implementation that accepts the HELLO command
/// and returns a basic RESP3 response
pub fn hello(
    cmd: ParsedCommand,
    cluster_holder: &Option<ClusterHolder>,
) -> Result<Response, anyhow::Error> {
    info!("HELLO: handling RESP3 handshake");

    // Parse protocol version if provided
    if cmd.argv.len() >= 2 {
        let protover = cmd.get_str(1)?;
        info!("HELLO: protocol version requested: {}", protover);
    }

    // Determine mode and role from cluster state
    let (mode, role) = match cluster_holder {
        // Synchronous peek at cluster state (block_on is fine here since it's quick)
        Some(_) => ("cluster", "master"),
        None => ("standalone", "master"),
    };

    // Return a basic RESP3 hello response
    // Format: map with server info
    Ok(Response::Array(vec![
        Response::Data(b"server".to_vec()),
        Response::Data(b"redis".to_vec()),
        Response::Data(b"version".to_vec()),
        Response::Data(b"999.999.999".to_vec()),
        Response::Data(b"proto".to_vec()),
        Response::Data(if cmd.argv.len() >= 2 {
            cmd.get_slice(1)?.to_vec()
        } else {
            b"3".to_vec()
        }),
        Response::Data(b"id".to_vec()),
        Response::Integer(1),
        Response::Data(b"mode".to_vec()),
        Response::Data(mode.as_bytes().to_vec()),
        Response::Data(b"role".to_vec()),
        Response::Data(role.as_bytes().to_vec()),
    ]))
}

/// AUTH [username] password
/// Authenticate the connection
///
/// For simplicity, we accept any authentication without actually checking
pub fn auth(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 2 {
        error!(
            "AUTH: wrong number of arguments (expected >= 2, got {})",
            cmd.argv.len()
        );
        return Err(anyhow!("ERR wrong number of arguments for 'AUTH' command"));
    }

    if cmd.argv.len() >= 3 {
        let username = cmd.get_str(1)?;
        let _password = cmd.get_str(2)?;
        info!("AUTH: username='{}', password='***'", username);
    } else {
        let _password = cmd.get_str(1)?;
        info!("AUTH: password='***'");
    }

    // Accept any authentication
    Ok(Response::Status("OK".to_owned()))
}

/// SELECT index
/// Select the database with the specified index
///
/// For simplicity, we acknowledge the command but don't actually switch databases
pub fn select_db(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() != 2 {
        error!(
            "SELECT: wrong number of arguments (expected 2, got {})",
            cmd.argv.len()
        );
        return Err(anyhow!(
            "ERR wrong number of arguments for 'SELECT' command"
        ));
    }

    let db_index = cmd.get_str(1)?;
    info!("SELECT: database index '{}'", db_index);

    // Parse and validate the index
    let index: u8 = db_index
        .parse()
        .map_err(|_| anyhow!("ERR invalid DB index"))?;

    if index > 15 {
        return Err(anyhow!("ERR DB index is out of range"));
    }

    // Accept the SELECT command
    Ok(Response::Status("OK".to_owned()))
}

/// INFO [section]
/// Return server information and statistics.
/// Supported sections: server, memory, keyspace, cluster, all (default)
pub fn info(
    cmd: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
    cluster_holder: &Option<ClusterHolder>,
) -> Result<Response, anyhow::Error> {
    let db = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;

    let (used_memory, max_memory) = {
        let lru = database_lock.lru_state.lock().map_err(|e| anyhow!("{}", e))?;
        (lru.memory_tracker.used_memory(), lru.memory_tracker.max_memory())
    };

    let section = if cmd.argv.len() >= 2 {
        cmd.get_str(1)?.to_lowercase()
    } else {
        "all".to_string()
    };

    let info_text = match section.as_str() {
        "server" => db.node_info.build_server_section(),
        "memory" => crate::database::info::NodeInfo::build_memory_section(used_memory, max_memory),
        "keyspace" => crate::database::info::NodeInfo::build_keyspace_section(&db, db_index),
        "cluster" => build_cluster_info_section(cluster_holder),
        _ => {
            let mut sections = vec![
                db.node_info.build_server_section(),
                crate::database::info::NodeInfo::build_memory_section(used_memory, max_memory),
                crate::database::info::NodeInfo::build_keyspace_section(&db, db_index),
            ];
            if cluster_holder.is_some() {
                sections.push(build_cluster_info_section(cluster_holder));
            }
            sections.join("\r\n")
        }
    };

    Ok(Response::Data(info_text.as_bytes().to_vec()))
}

/// Build the # Cluster section for the INFO command.
fn build_cluster_info_section(cluster_holder: &Option<ClusterHolder>) -> String {
    match cluster_holder {
        Some(_) => {
            // Note: We can't async-read the cluster state here since this is a sync function.
            // For a more complete implementation, we'd need to make info() async.
            // For now, return basic cluster info.
            "# Cluster\r\ncluster_enabled:1".to_string()
        }
        None => "# Cluster\r\ncluster_enabled:0".to_string(),
    }
}
