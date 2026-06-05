use crate::cluster::node::ClusterNode;
use crate::cluster::state::ClusterHolder;
use crate::cluster::transport::{build_cluster_message, build_state_digest, parse_state_digest};
use crate::parser::request::Request;
use chrono::Utc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, interval};

/// Send a MEET message directly to a target node's bus port.
/// Used by the CLUSTER MEET command.
pub async fn send_meet_direct(
    target_bus_addr: &str,
    sender_id: &str,
    sender_client_addr: &str,
) -> Result<(), anyhow::Error> {
    let msg = build_cluster_message(&[
        b"MEET",
        sender_id.as_bytes(),
        sender_client_addr.as_bytes(),
    ]);
    let _response = crate::cluster::transport::send_cluster_message(target_bus_addr, &msg).await?;
    Ok(())
}

/// Start the gossip loop — periodically PING a random known node.
pub async fn start_gossip_loop(cluster_holder: ClusterHolder) {
    let mut tick = interval(Duration::from_secs(1));

    loop {
        tick.tick().await;

        // Pick a random node to PING (excluding myself)
        let target = {
            let state = cluster_holder.state.read().await;
            let candidates: Vec<_> = state
                .nodes
                .values()
                .filter(|n| !n.is_myself() && n.connected)
                .collect();

            if candidates.is_empty() {
                continue;
            }

            // Simple "random": use current second modulo count
            let idx = (Utc::now().timestamp_millis().unsigned_abs() as usize) % candidates.len();
            let target = candidates[idx];
            (
                target.id.clone(),
                format!("{}:{}", target.addr.ip(), target.bus_port()),
            )
        };

        // Build and send PING
        let digest_bytes = {
            let state = cluster_holder.state.read().await;
            build_state_digest(&state)
        };

        let sender_id = {
            let state = cluster_holder.state.read().await;
            state.myself.id.clone()
        };

        let ping_msg = build_cluster_message(&[
            b"PING",
            sender_id.as_bytes(),
            &digest_bytes,
        ]);

        // Update ping_sent
        {
            let mut state = cluster_holder.state.write().await;
            let now = Utc::now().timestamp();
            state.myself.ping_sent = now;
        }

        match crate::cluster::transport::send_cluster_message(&target.1, &ping_msg).await {
            Ok(response_data) => {
                // Parse PONG response
                if let Ok(commands) = Request::parse_all(&response_data) {
                    if commands.len() >= 3 {
                        if let Ok(msg_type) = commands[0].get_str(0) {
                            if msg_type.eq_ignore_ascii_case("PONG") {
                                if let Ok(digest_data) = commands[2].get_vec(0) {
                                    match parse_state_digest(&digest_data) {
                                        Ok((sender_id, sender_addr, epoch, node_digests)) => {
                                            let mut state = cluster_holder.state.write().await;
                                                state.merge_state(
                                                    &sender_id,
                                                    sender_addr,
                                                    epoch,
                                                    &node_digests,
                                                );
                                            info!(
                                                "Gossip: merged state from node {} (epoch={})",
                                                sender_id, epoch
                                            );
                                        }
                                        Err(e) => {
                                            debug!("Gossip: failed to parse PONG digest: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                debug!(
                    "Gossip: failed to PING node {} at {}: {}",
                    target.0, target.1, e
                );
            }
        }
    }
}

/// Start the cluster bus listener — accepts connections from other nodes.
pub async fn start_gossip_listener(cluster_holder: ClusterHolder, bus_port: u16) {
    let addr = format!("0.0.0.0:{}", bus_port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            info!("Cluster bus listening on {}", addr);
            l
        }
        Err(e) => {
            error!("Failed to bind cluster bus on {}: {}", addr, e);
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let ch = cluster_holder.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_gossip_connection(stream, peer_addr, &ch).await {
                        debug!("Gossip connection error from {}: {}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                error!("Cluster bus accept error: {}", e);
            }
        }
    }
}

/// Handle a single gossip connection (MEET or PING).
async fn handle_gossip_connection(
    mut stream: tokio::net::TcpStream,
    _peer_addr: std::net::SocketAddr,
    cluster_holder: &ClusterHolder,
) -> Result<(), anyhow::Error> {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    buf.truncate(n);

    let commands = Request::parse_all(&buf)?;
    if commands.is_empty() {
        return Ok(());
    }

    let msg_type = commands[0]
        .get_str(0)
        .unwrap_or("")
        .to_uppercase();

    match msg_type.as_str() {
        "MEET" => {
            handle_meet(&commands, cluster_holder).await?;
            // Respond with PONG + our state digest
            let pong = build_pong_response(cluster_holder).await;
            stream.write_all(&pong).await?;
        }
        "PING" => {
            // Parse incoming state and merge
            if commands.len() >= 3 {
                if let Ok(digest_data) = commands[0].get_vec(2) {
                    match parse_state_digest(&digest_data) {
                        Ok((sender_id, sender_addr, epoch, node_digests)) => {
                            let mut state = cluster_holder.state.write().await;
                            state.merge_state(&sender_id, sender_addr, epoch, &node_digests);
                            info!(
                                "Gossip: received PING from node {} (epoch={})",
                                sender_id, epoch
                            );
                        }
                        Err(e) => {
                            debug!("Gossip: failed to parse PING digest: {}", e);
                        }
                    }
                }
            }
            // Respond with PONG + our state digest
            let pong = build_pong_response(cluster_holder).await;
            stream.write_all(&pong).await?;
        }
        _ => {
            debug!("Gossip: unknown message type: {}", msg_type);
        }
    }

    Ok(())
}

/// Handle a MEET message: add the sender as a known node.
async fn handle_meet(
    commands: &[crate::vojo::parsered_command::ParsedCommand],
    cluster_holder: &ClusterHolder,
) -> Result<(), anyhow::Error> {
    if commands[0].argv.len() < 3 {
        return Err(anyhow::anyhow!("MEET requires at least 3 arguments"));
    }

    let sender_id = commands[0].get_str(1)?.to_string();
    let sender_addr_str = commands[0].get_str(2)?;
    let sender_addr: std::net::SocketAddr = sender_addr_str.parse()?;

    let mut state = cluster_holder.state.write().await;

    if !state.nodes.contains_key(&sender_id) {
        let new_node = ClusterNode::new(sender_id.clone(), sender_addr);
        state.add_node(new_node);
        info!("Gossip: MEET — added node {} at {}", sender_id, sender_addr);
    } else {
        info!("Gossip: MEET — node {} already known", sender_id);
    }

    Ok(())
}

/// Build a PONG response with our current cluster state digest.
async fn build_pong_response(cluster_holder: &ClusterHolder) -> Vec<u8> {
    let state = cluster_holder.state.read().await;
    let sender_id = state.myself.id.clone();
    let digest = build_state_digest(&state);
    build_cluster_message(&[b"PONG", sender_id.as_bytes(), &digest])
}
