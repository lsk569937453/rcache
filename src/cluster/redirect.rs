use crate::cluster::slot::key_hash_slot;
use crate::cluster::state::ClusterHolder;
use crate::parser::response::Response;

/// Returns the argument index of the key for a given command name.
/// Returns `None` for commands that don't operate on keys.
pub fn key_index_for_command(cmd: &str) -> Option<usize> {
    match cmd {
        // String commands
        "GET" | "SET" | "APPEND" | "INCR" | "DECR" | "INCRBY" | "DECRBY"
        | "INCRBYFLOAT" | "STRLEN" | "GETRANGE" | "GETSET" | "GETDEL" => Some(1),
        // Key commands
        "DEL" | "EXISTS" | "TYPE" | "EXPIRE" | "TTL" | "DBSIZE" | "KEYS" => Some(1),
        // Multi-key commands (use first key)
        "MGET" | "MSET" | "MSETNX" => Some(1),
        // List commands
        "LPUSH" | "RPUSH" | "LPOP" | "RPOP" | "LRANGE" | "LLEN" => Some(1),
        // Set commands
        "SADD" | "SREM" | "SMEMBERS" | "SISMEMBER" | "SCARD" => Some(1),
        // Hash commands
        "HSET" | "HGET" | "HGETALL" | "HDEL" | "HEXISTS" | "HLEN" => Some(1),
        // Sorted set commands
        "ZADD" | "ZRANGE" | "ZREM" | "ZCARD" | "ZSCORE" => Some(1),
        // Commands that don't need routing
        _ => None,
    }
}

/// Check if a command should be redirected in cluster mode.
///
/// Returns `Ok(())` if the command can proceed on this node.
/// Returns `Err(Response)` with a MOVED redirect or CLUSTERDOWN error
/// if the key belongs to a different node or an unassigned slot.
pub async fn check_cluster_redirect(
    _command_name: &str,
    key: &[u8],
    cluster_holder: &ClusterHolder,
) -> Result<(), Response> {
    let state = cluster_holder.state.read().await;
    let slot = key_hash_slot(key);

    match &state.slots.get(slot as usize) {
        Some(Some(owner_id)) => {
            if owner_id == &state.myself.id {
                // Slot belongs to this node — proceed
                Ok(())
            } else {
                // Slot belongs to another node — MOVED redirect
                let owner_node = state.nodes.get(owner_id);
                match owner_node {
                    Some(node) => Err(Response::Error(format!(
                        "MOVED {} {}:{}",
                        slot,
                        node.addr.ip(),
                        node.addr.port()
                    ))),
                    None => Err(Response::Error(format!(
                        "MOVED {} unknown:0",
                        slot
                    ))),
                }
            }
        }
        _ => {
            // Slot is not assigned — CLUSTERDOWN
            Err(Response::Error(format!(
                "CLUSTERDOWN Hash slot {} is not served",
                slot
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_index_for_command() {
        assert_eq!(key_index_for_command("GET"), Some(1));
        assert_eq!(key_index_for_command("SET"), Some(1));
        assert_eq!(key_index_for_command("LPUSH"), Some(1));
        assert_eq!(key_index_for_command("HSET"), Some(1));
        assert_eq!(key_index_for_command("MGET"), Some(1));
        assert_eq!(key_index_for_command("PING"), None);
        assert_eq!(key_index_for_command("CLUSTER"), None);
        assert_eq!(key_index_for_command("INFO"), None);
        assert_eq!(key_index_for_command("AUTH"), None);
        assert_eq!(key_index_for_command("SELECT"), None);
        assert_eq!(key_index_for_command("CLIENT"), None);
        assert_eq!(key_index_for_command("HELLO"), None);
    }
}
