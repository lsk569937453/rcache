use std::collections::{BTreeSet, HashMap};

use crate::vojo::value::Value;

/// LRU tracker for a single database shard.
/// Maintains access order using a monotonic clock and BTreeSet for O(log n) eviction.
pub struct LruTracker {
    /// Maps key -> last access clock value
    access_map: HashMap<Vec<u8>, u64>,
    /// Ordered set of (clock_value, key) for efficient min lookup
    access_order: BTreeSet<(u64, Vec<u8>)>,
    /// Monotonically increasing clock
    clock: u64,
}

impl LruTracker {
    pub fn new() -> Self {
        LruTracker {
            access_map: HashMap::new(),
            access_order: BTreeSet::new(),
            clock: 0,
        }
    }

    /// Record access to a key (call on every read and write). O(log n)
    pub fn touch(&mut self, key: &[u8]) {
        self.clock += 1;
        // Remove old entry if it exists
        if let Some(&old_clock) = self.access_map.get(key) {
            self.access_order.remove(&(old_clock, key.to_vec()));
        }
        // Insert new entry
        self.access_map.insert(key.to_vec(), self.clock);
        self.access_order.insert((self.clock, key.to_vec()));
    }

    /// Remove a key from tracking (call on delete). O(log n)
    pub fn remove(&mut self, key: &[u8]) {
        if let Some(old_clock) = self.access_map.remove(key) {
            self.access_order.remove(&(old_clock, key.to_vec()));
        }
    }

    /// Pop (remove and return) the least recently used key. O(log n)
    pub fn pop_lru(&mut self) -> Option<Vec<u8>> {
        let first = self.access_order.iter().next().cloned();
        if let Some((clock, key)) = first {
            self.access_order.remove(&(clock, key.clone()));
            self.access_map.remove(&key);
            Some(key)
        } else {
            None
        }
    }

    /// Get the least recently used key without removing it.
    pub fn lru_key(&self) -> Option<&Vec<u8>> {
        self.access_order.iter().next().map(|(_, key)| key)
    }

    /// Get the least recently used key with its clock value.
    pub fn lru_key_with_clock(&self) -> Option<(u64, &Vec<u8>)> {
        self.access_order.iter().next().map(|(clock, key)| (*clock, key))
    }

    /// Clear all tracking data.
    pub fn clear(&mut self) {
        self.access_map.clear();
        self.access_order.clear();
        self.clock = 0;
    }
}

/// Memory tracker for the entire database.
pub struct MemoryTracker {
    /// Current estimated memory usage in bytes
    pub used_memory: usize,
    /// Maximum allowed memory in bytes (0 = unlimited)
    pub max_memory: usize,
}

impl MemoryTracker {
    pub fn new(max_memory: usize) -> Self {
        MemoryTracker {
            used_memory: 0,
            max_memory,
        }
    }

    pub fn is_over_limit(&self) -> bool {
        self.max_memory > 0 && self.used_memory > self.max_memory
    }

    pub fn add(&mut self, bytes: usize) {
        self.used_memory += bytes;
    }

    pub fn sub(&mut self, bytes: usize) {
        self.used_memory = self.used_memory.saturating_sub(bytes);
    }

    pub fn used_memory(&self) -> usize {
        self.used_memory
    }

    pub fn max_memory(&self) -> usize {
        self.max_memory
    }
}

/// Estimate memory usage of a Value in bytes.
pub fn estimate_value_memory(value: &Value) -> usize {
    match value {
        Value::Nil => 0,
        Value::String(s) => s.data.len() + 8,
        Value::List(l) => l.data.iter().map(|item| item.len()).sum::<usize>() + 16,
        Value::Set(s) => {
            s.data.iter().map(|item| item.len()).sum::<usize>() + s.data.len() * 8 + 16
        }
        Value::Hash(h) => {
            h.data
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
                + h.data.len() * 16
                + 16
        }
        Value::SortedSet(z) => {
            z.data
                .iter()
                .map(|item| item.member.len() + 8)
                .sum::<usize>()
                + 16
        }
    }
}
