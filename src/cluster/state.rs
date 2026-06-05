use crate::cluster::node::{generate_node_id, ClusterNode, NodeFlags, SlotRange};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cluster health state.
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterHealth {
    Ok,
    Fail,
}

impl std::fmt::Display for ClusterHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClusterHealth::Ok => write!(f, "ok"),
            ClusterHealth::Fail => write!(f, "fail"),
        }
    }
}

/// The cluster state maintained by each node.
#[derive(Debug, Clone)]
pub struct ClusterState {
    /// This node's information.
    pub myself: ClusterNode,
    /// All known nodes: node_id -> ClusterNode.
    pub nodes: HashMap<String, ClusterNode>,
    /// Slot-to-owner mapping: slot index -> node_id.
    /// 16384 entries, one per slot. Boxed to avoid stack overflow.
    pub slots: Box<[Option<String>; CLUSTER_HASH_SLOTS as usize]>,
    /// The current configuration epoch.
    pub config_epoch: u64,
    /// Overall cluster health.
    pub health: ClusterHealth,
}

/// Total number of hash slots.
pub const CLUSTER_HASH_SLOTS: u16 = 16384;

/// Shared cluster state holder.
#[derive(Clone)]
pub struct ClusterHolder {
    pub state: Arc<RwLock<ClusterState>>,
}

impl ClusterHolder {
    pub fn new(state: ClusterState) -> Self {
        ClusterHolder {
            state: Arc::new(RwLock::new(state)),
        }
    }
}

impl ClusterState {
    /// Create a new cluster state with only the "myself" node.
    pub fn new(my_addr: SocketAddr, timestamp: i64) -> Self {
        let node_id = generate_node_id(&my_addr, timestamp);
        let myself = ClusterNode::new_myself(node_id.clone(), my_addr);

        let mut nodes = HashMap::new();
        nodes.insert(node_id.clone(), myself.clone());

        // Initialize all slots as unassigned (boxed to avoid stack overflow)
        let slots = Box::new(std::array::from_fn(|_: usize| None::<String>));

        ClusterState {
            myself,
            nodes,
            slots,
            config_epoch: 0,
            health: ClusterHealth::Fail, // Start as Fail until slots are assigned
        }
    }

    /// Get the node ID of the local node.
    pub fn my_id(&self) -> &str {
        &self.myself.id
    }

    /// Get the client-facing port of the local node.
    pub fn my_port(&self) -> u16 {
        self.myself.addr.port()
    }

    /// Add a node to the cluster (from CLUSTER MEET or gossip).
    pub fn add_node(&mut self, node: ClusterNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&mut self, node_id: &str) {
        if let Some(_removed) = self.nodes.remove(node_id) {
            // Clear any slots owned by this node
            for slot in self.slots.iter_mut() {
                if slot.as_deref() == Some(node_id) {
                    *slot = None;
                }
            }
        }
    }

    /// Assign a set of slot ranges to a specific node.
    pub fn add_slots(&mut self, ranges: &[SlotRange], node_id: &str) {
        for range in ranges {
            for slot in range.start..=range.end {
                if (slot as usize) < self.slots.len() {
                    self.slots[slot as usize] = Some(node_id.to_string());
                }
            }
        }

        // Update the node's slot_ranges
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.slot_ranges.extend(ranges.iter().cloned());
            node.slot_ranges.sort_by_key(|r| r.start);
        }

        // Update myself too if applicable
        if node_id == self.myself.id {
            self.myself.slot_ranges.extend(ranges.iter().cloned());
            self.myself.slot_ranges.sort_by_key(|r| r.start);
        }

        self.update_health();
    }

    /// Remove specific slots from a node.
    pub fn del_slots(&mut self, slots: &[u16]) {
        for &slot in slots {
            if (slot as usize) < self.slots.len() {
                if let Some(ref owner_id) = self.slots[slot as usize] {
                    if let Some(node) = self.nodes.get_mut(owner_id) {
                        node.slot_ranges
                            .retain(|r| slot < r.start || slot > r.end);
                    }
                    self.slots[slot as usize] = None;
                }
            }
        }
        self.update_health();
    }

    /// Remove all slot assignments.
    pub fn flush_slots(&mut self) {
        for slot in self.slots.iter_mut() {
            *slot = None;
        }
        for node in self.nodes.values_mut() {
            node.slot_ranges.clear();
        }
        self.myself.slot_ranges.clear();
        self.health = ClusterHealth::Fail;
    }

    /// Look up the owner of a given slot.
    pub fn slot_owner(&self, slot: u16) -> Option<&ClusterNode> {
        if (slot as usize) >= self.slots.len() {
            return None;
        }
        match &self.slots[slot as usize] {
            Some(node_id) => self.nodes.get(node_id),
            None => None,
        }
    }

    /// Check if a slot belongs to the local node.
    pub fn is_slot_mine(&self, slot: u16) -> bool {
        match &self.slots.get(slot as usize) {
            Some(Some(owner_id)) => *owner_id == self.myself.id,
            _ => false,
        }
    }

    /// Count how many slots are assigned.
    pub fn assigned_slot_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Count the number of master nodes.
    pub fn master_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| n.is_master())
            .count()
    }

    /// Reset cluster state to a fresh state (keep only myself).
    pub fn reset(&mut self) {
        let myself_id = self.myself.id.clone();
        let myself_addr = self.myself.addr;

        let myself = ClusterNode::new_myself(myself_id, myself_addr);
        let mut nodes = HashMap::new();
        nodes.insert(myself.id.clone(), myself.clone());

        self.myself = myself;
        self.nodes = nodes;
        self.slots = Box::new(std::array::from_fn(|_: usize| None::<String>));
        self.config_epoch = 0;
        self.health = ClusterHealth::Fail;
    }

    /// Merge state received from another node via gossip.
    ///
    /// Rules:
    /// - New nodes are added.
    /// - For existing nodes, the one with higher config_epoch wins for slot assignments.
    /// - Global config_epoch takes the max.
    pub fn merge_state(
        &mut self,
        sender_id: &str,
        sender_addr: SocketAddr,
        sender_epoch: u64,
        node_digests: &[(String, SocketAddr, Vec<NodeFlags>, Vec<SlotRange>, u64)],
    ) {
        // Add or update sender
        if !self.nodes.contains_key(sender_id) {
            let sender_node = ClusterNode::new(sender_id.to_string(), sender_addr);
            self.nodes.insert(sender_id.to_string(), sender_node);
        }

        // Update sender's pong_received
        if let Some(node) = self.nodes.get_mut(sender_id) {
            node.pong_received = chrono::Utc::now().timestamp();
            node.connected = true;
        }

        // Merge each node digest
        for (node_id, addr, flags, slot_ranges, epoch) in node_digests {
            if !self.nodes.contains_key(node_id) {
                let mut new_node = ClusterNode::new(node_id.clone(), *addr);
                new_node.flags = flags.clone();
                new_node.config_epoch = *epoch;
                new_node.slot_ranges = slot_ranges.clone();
                self.nodes.insert(node_id.clone(), new_node);
            } else if let Some(existing) = self.nodes.get_mut(node_id) {
                // Update if the incoming epoch is higher
                if *epoch > existing.config_epoch {
                    existing.config_epoch = *epoch;
                    existing.slot_ranges = slot_ranges.clone();
                    existing.flags = flags.clone();
                    existing.addr = *addr;
                }
            }

            // Apply slot assignments from higher epoch
            if *epoch >= self.config_epoch {
                for range in slot_ranges {
                    for slot in range.start..=range.end {
                        if (slot as usize) < self.slots.len() {
                            // Only update if the new epoch is higher or equal
                            match &self.slots[slot as usize] {
                                None => {
                                    self.slots[slot as usize] = Some(node_id.clone());
                                }
                                Some(current_owner) => {
                                    // Check current owner's epoch
                                    let current_epoch = self
                                        .nodes
                                        .get(current_owner)
                                        .map(|n| n.config_epoch)
                                        .unwrap_or(0);
                                    if *epoch > current_epoch {
                                        self.slots[slot as usize] = Some(node_id.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Update global epoch
        if sender_epoch > self.config_epoch {
            self.config_epoch = sender_epoch;
        }

        self.update_health();
    }

    /// Update the cluster health based on slot coverage.
    pub fn update_health(&mut self) {
        // Cluster is OK when all 16384 slots are assigned
        let assigned = self.assigned_slot_count();
        self.health = if assigned == CLUSTER_HASH_SLOTS as usize {
            ClusterHealth::Ok
        } else {
            ClusterHealth::Fail
        };
    }

    /// Build the CLUSTER SLOTS response.
    /// Returns a Vec of (start_slot, end_slot, [(node_id, ip, port)]) tuples.
    pub fn cluster_slots_info(&self) -> Vec<(u16, u16, Vec<(String, String, u16)>)> {
        let mut result = Vec::new();
        let mut i: usize = 0;

        while i < self.slots.len() {
            match &self.slots[i] {
                None => {
                    i += 1;
                }
                Some(owner_id) => {
                    let start = i as u16;
                    let mut end = start;

                    // Find consecutive slots with the same owner
                    while end as usize + 1 < self.slots.len()
                        && self.slots[end as usize + 1].as_deref() == Some(owner_id)
                    {
                        end += 1;
                    }

                    if let Some(node) = self.nodes.get(owner_id) {
                        let node_info = (
                            node.id.clone(),
                            node.addr.ip().to_string(),
                            node.addr.port(),
                        );
                        result.push((start, end, vec![node_info]));
                    }

                    i = end as usize + 1;
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    #[test]
    fn test_new_cluster_state() {
        let state = ClusterState::new(test_addr(6370), 1000);
        assert_eq!(state.nodes.len(), 1);
        assert!(state.myself.is_myself());
        assert_eq!(state.health, ClusterHealth::Fail);
        assert_eq!(state.assigned_slot_count(), 0);
    }

    #[test]
    fn test_add_slots() {
        let mut state = ClusterState::new(test_addr(6370), 1000);
        let my_id = state.my_id().to_string();

        state.add_slots(
            &[SlotRange {
                start: 0,
                end: 16383,
            }],
            &my_id,
        );

        assert_eq!(state.assigned_slot_count(), 16384);
        assert_eq!(state.health, ClusterHealth::Ok);
        assert!(state.is_slot_mine(0));
        assert!(state.is_slot_mine(8192));
        assert!(state.is_slot_mine(16383));
    }

    #[test]
    fn test_slot_owner() {
        let mut state = ClusterState::new(test_addr(6370), 1000);
        let my_id = state.my_id().to_string();

        state.add_slots(
            &[SlotRange {
                start: 0,
                end: 100,
            }],
            &my_id,
        );

        let owner = state.slot_owner(50).unwrap();
        assert_eq!(owner.id, my_id);

        assert!(state.slot_owner(200).is_none());
    }

    #[test]
    fn test_del_slots() {
        let mut state = ClusterState::new(test_addr(6370), 1000);
        let my_id = state.my_id().to_string();

        state.add_slots(
            &[SlotRange {
                start: 0,
                end: 100,
            }],
            &my_id,
        );

        state.del_slots(&[50, 51]);
        assert!(state.slot_owner(50).is_none());
        assert!(state.slot_owner(51).is_none());
        assert_eq!(state.assigned_slot_count(), 99);
    }

    #[test]
    fn test_flush_slots() {
        let mut state = ClusterState::new(test_addr(6370), 1000);
        let my_id = state.my_id().to_string();

        state.add_slots(
            &[SlotRange {
                start: 0,
                end: 16383,
            }],
            &my_id,
        );
        assert_eq!(state.health, ClusterHealth::Ok);

        state.flush_slots();
        assert_eq!(state.assigned_slot_count(), 0);
        assert_eq!(state.health, ClusterHealth::Fail);
    }

    #[test]
    fn test_reset() {
        let mut state = ClusterState::new(test_addr(6370), 1000);
        let my_id = state.my_id().to_string();

        state.add_slots(
            &[SlotRange {
                start: 0,
                end: 16383,
            }],
            &my_id,
        );

        // Add another node
        let other = ClusterNode::new("other_node_id".to_string(), test_addr(6371));
        state.add_node(other);

        assert_eq!(state.nodes.len(), 2);

        state.reset();
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.assigned_slot_count(), 0);
    }

    #[test]
    fn test_cluster_slots_info() {
        let mut state = ClusterState::new(test_addr(6370), 1000);
        let my_id = state.my_id().to_string();

        state.add_slots(
            &[SlotRange {
                start: 0,
                end: 5460,
            }],
            &my_id,
        );

        let info = state.cluster_slots_info();
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].0, 0);
        assert_eq!(info[0].1, 5460);
    }
}
