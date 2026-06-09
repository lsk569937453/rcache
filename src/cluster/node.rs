use std::fmt;
use std::net::SocketAddr;

/// Flags describing the state/role of a cluster node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeFlags {
    Myself,
    Master,
    Slave,
    Handshake,
    NoAddr,
}

impl fmt::Display for NodeFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeFlags::Myself => write!(f, "myself"),
            NodeFlags::Master => write!(f, "master"),
            NodeFlags::Slave => write!(f, "slave"),
            NodeFlags::Handshake => write!(f, "handshake"),
            NodeFlags::NoAddr => write!(f, "noaddr"),
        }
    }
}

/// A range of hash slots: [start, end] inclusive.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotRange {
    pub start: u16,
    pub end: u16,
}

impl fmt::Display for SlotRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

/// Represents a single node in the cluster.
#[derive(Debug, Clone)]
pub struct ClusterNode {
    /// 40-character hex node ID.
    pub id: String,
    /// Client-facing address (ip:port).
    pub addr: SocketAddr,
    /// Node flags (myself, master, slave, etc.).
    pub flags: Vec<NodeFlags>,
    /// For slave nodes, the master's node ID.
    pub master_id: Option<String>,
    /// Unix timestamp of last PING sent.
    pub ping_sent: i64,
    /// Unix timestamp of last PONG received.
    pub pong_received: i64,
    /// Configuration epoch.
    pub config_epoch: u64,
    /// Slots assigned to this node.
    pub slot_ranges: Vec<SlotRange>,
    /// Whether this node is connected.
    pub connected: bool,
}

impl ClusterNode {
    /// Create a new cluster node with the given ID and address.
    pub fn new(id: String, addr: SocketAddr) -> Self {
        ClusterNode {
            id,
            addr,
            flags: vec![NodeFlags::Master],
            master_id: None,
            ping_sent: 0,
            pong_received: 0,
            config_epoch: 0,
            slot_ranges: Vec::new(),
            connected: true,
        }
    }

    /// Create a "myself" node (the local node).
    pub fn new_myself(id: String, addr: SocketAddr) -> Self {
        ClusterNode {
            flags: vec![NodeFlags::Myself, NodeFlags::Master],
            ..Self::new(id, addr)
        }
    }

    /// Check if this is the "myself" node.
    pub fn is_myself(&self) -> bool {
        self.flags.contains(&NodeFlags::Myself)
    }

    /// Check if this node is a master.
    pub fn is_master(&self) -> bool {
        self.flags.contains(&NodeFlags::Master)
            || (self.flags.contains(&NodeFlags::Myself) && self.master_id.is_none())
    }

    /// Format the node as a CLUSTER NODES line (Redis-compatible format).
    ///
    /// Format: `<id> <ip:port@cport> <flags> <master_id> <ping_sent> <pong_recv> <config_epoch> <link_state> <slot_ranges>`
    pub fn to_cluster_nodes_line(&self, bus_port: u16) -> String {
        let flags_str = self
            .flags
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let master_id_str = self
            .master_id
            .as_deref()
            .unwrap_or("-");

        let link_state = if self.connected { "connected" } else { "disconnected" };

        let mut line = format!(
            "{} {}@{} {} {} {} {} {} {}",
            self.id,
            self.addr.ip(),
            self.addr.port(),
            bus_port,
            flags_str,
            master_id_str,
            self.ping_sent,
            self.pong_received,
            self.config_epoch,
        );
        line.push(' ');
        line.push_str(link_state);

        for range in &self.slot_ranges {
            line.push(' ');
            line.push_str(&range.to_string());
        }

        line
    }

    /// Get the cluster bus port (client port + 10000).
    pub fn bus_port(&self) -> u16 {
        self.addr.port().saturating_add(10000)
    }
}

/// Generate a 40-character hex node ID from an address and timestamp.
///
/// Uses a simple FNV-1a hash approach to produce a deterministic-looking ID.
pub fn generate_node_id(addr: &SocketAddr, timestamp: i64) -> String {
    // Simple deterministic ID generation: FNV-1a hash of addr + timestamp,
    // repeated to fill 40 hex chars (20 bytes).
    let mut result = String::with_capacity(40);
    let input = format!("{}:{}", addr, timestamp);

    for round in 0..5 {
        let mut hash: u64 = 0xcbf29ce484222325;
        let round_bytes = format!("{}{}", input, round);
        for &byte in round_bytes.as_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        result.push_str(&format!("{:016x}", hash));
    }

    result.truncate(40);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_generate_node_id_length() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 6370);
        let id = generate_node_id(&addr, 1234567890);
        assert_eq!(id.len(), 40);
    }

    #[test]
    fn test_generate_node_id_deterministic() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 6370);
        let id1 = generate_node_id(&addr, 1234567890);
        let id2 = generate_node_id(&addr, 1234567890);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_cluster_nodes_line() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 6370);
        let node = ClusterNode::new_myself("abc123".to_string(), addr);
        let line = node.to_cluster_nodes_line(16370);
        assert!(line.starts_with("abc123"));
        assert!(line.contains("myself,master"));
        assert!(line.contains("connected"));
    }

    #[test]
    fn test_slot_range_display() {
        assert_eq!(SlotRange { start: 0, end: 5460 }.to_string(), "0-5460");
        assert_eq!(SlotRange { start: 100, end: 100 }.to_string(), "100");
    }

    #[test]
    fn test_bus_port() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 6370);
        let node = ClusterNode::new_myself("test".to_string(), addr);
        assert_eq!(node.bus_port(), 16370);
    }
}
