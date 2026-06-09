use crate::database::fs_writer::MyWriter;
use crate::database::lru::{estimate_value_memory, LruTracker, MemoryTracker};
use crate::database::aof::AofWriter;
use crate::parser::response::Response;

use crate::vojo::value::BackgroundEvent;
use crate::vojo::value::Value;
use crate::vojo::value::{ValueSet, ValueSortedSet, ValueString};

use std::collections::{BTreeSet, HashMap};
use std::collections::{HashSet, VecDeque};

use super::info::NodeInfo;
use crate::logger::default_logger::setup_logger;
use crate::vojo::value::ValueHash;
use crate::vojo::value::ValueList;
use bincode::{config, Decode, Encode};
use chrono::Utc;
#[cfg(not(any(target_os = "windows")))]
use fork::fork;
#[cfg(not(any(target_os = "windows")))]
use fork::Fork;
use std::fs::OpenOptions;
#[cfg(not(any(target_os = "windows")))]
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio::time::Instant;

/// LRU and memory tracking state, kept separate from Database
/// to preserve RDB serialization compatibility.
pub struct LruState {
    pub lru_trackers: Vec<LruTracker>,
    pub memory_tracker: MemoryTracker,
}

#[derive(Clone)]
pub struct DatabaseHolder {
    pub database_lock: Arc<Mutex<Database>>,
    pub aof_writer: Option<Arc<AofWriter>>,
    pub lru_state: Arc<Mutex<LruState>>,
}
impl DatabaseHolder {
    /// Log a write command to AOF if AOF is enabled.
    pub fn log_to_aof(&self, resp_bytes: &[u8]) -> Result<(), anyhow::Error> {
        if let Some(ref writer) = self.aof_writer {
            writer.append(resp_bytes)?;
        }
        Ok(())
    }

    /// Initialize LRU trackers to match the current database keys.
    /// Called after loading RDB or AOF.
    pub fn init_lru_from_database(&self) {
        let db = self.database_lock.lock().unwrap();
        let mut state = self.lru_state.lock().unwrap();
        // Track existing keys and estimate memory
        for shard_idx in 0..db.data.len() {
            for (key, value) in &db.data[shard_idx] {
                state.lru_trackers[shard_idx].touch(key);
                let mem = estimate_value_memory(value);
                state.memory_tracker.add(mem);
            }
        }
    }

    /// Touch a key for LRU tracking (record access).
    pub fn lru_touch(&self, db_index: usize, key: &[u8]) {
        if let Ok(mut state) = self.lru_state.lock() {
            if let Some(tracker) = state.lru_trackers.get_mut(db_index) {
                tracker.touch(key);
            }
        }
    }

    /// Remove a key from LRU tracking.
    pub fn lru_remove(&self, db_index: usize, key: &[u8]) {
        if let Ok(mut state) = self.lru_state.lock() {
            if let Some(tracker) = state.lru_trackers.get_mut(db_index) {
                tracker.remove(key);
            }
        }
    }

    /// Add to memory usage estimate.
    pub fn memory_add(&self, bytes: usize) {
        if let Ok(mut state) = self.lru_state.lock() {
            state.memory_tracker.add(bytes);
        }
    }

    /// Subtract from memory usage estimate.
    pub fn memory_sub(&self, bytes: usize) {
        if let Ok(mut state) = self.lru_state.lock() {
            state.memory_tracker.sub(bytes);
        }
    }

    /// Check if memory is over limit.
    pub fn is_memory_over_limit(&self) -> bool {
        if let Ok(state) = self.lru_state.lock() {
            state.memory_tracker.is_over_limit()
        } else {
            false
        }
    }

    /// Evict keys until memory usage is below the limit.
    /// IMPORTANT: caller must NOT hold database_lock or lru_state lock.
    pub fn evict_if_needed(&self) -> Result<(), anyhow::Error> {
        loop {
            // Check if eviction is needed
            {
                let state = self.lru_state.lock().map_err(|e| anyhow!("{}", e))?;
                if !state.memory_tracker.is_over_limit() {
                    return Ok(());
                }
            }

            // Find the global LRU key across all shards
            let evict_info = {
                let state = self.lru_state.lock().map_err(|e| anyhow!("{}", e))?;
                let mut best: Option<(u64, usize, Vec<u8>)> = None;
                for (idx, tracker) in state.lru_trackers.iter().enumerate() {
                    if let Some((clock, key)) = tracker.lru_key_with_clock() {
                        match &best {
                            Some(b) if clock >= b.0 => {}
                            _ => best = Some((clock, idx, key.clone())),
                        }
                    }
                }
                best
            };

            match evict_info {
                Some((_, shard_idx, key)) => {
                    // Remove from database and LRU
                    let mut db = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
                    if let Some(value) = db.data[shard_idx].remove(&key) {
                        let mem = estimate_value_memory(&value);
                        self.memory_sub(mem);
                    }
                    db.expire_map[shard_idx].remove(&key);
                    drop(db);
                    self.lru_remove(shard_idx, &key);
                    info!("LRU evicted key {:?} from shard {}", key, shard_idx);
                }
                None => return Ok(()), // no keys to evict
            }
        }
    }
    pub async fn expire_loop(&self) -> Result<(), anyhow::Error> {
        let mut interval = interval(Duration::from_millis(200));
        loop {
            interval.tick().await;

            let mut lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
            let current_timestamp = Utc::now().timestamp();

            let expired_keys_by_index: Vec<Vec<Vec<u8>>> = lock
                .expire_map
                .iter()
                .map(|map| {
                    map.iter()
                        .filter(|(_, &expire_at)| expire_at <= current_timestamp)
                        .map(|(key, _)| key.clone())
                        .collect()
                })
                .collect();

            for (index, expired_keys) in expired_keys_by_index.into_iter().enumerate() {
                for key in &expired_keys {
                    debug!(
                        "the key |{:?}| in slot {} has been removed",
                        key.clone(),
                        index
                    );
                    // Estimate memory before removing
                    let mem = lock.data[index]
                        .get(key)
                        .map(|v| estimate_value_memory(v))
                        .unwrap_or(0);
                    lock.expire_map[index].remove(key);
                    lock.data[index].remove(key);
                    // Update LRU and memory tracking
                    self.memory_sub(mem);
                    drop(lock);
                    self.lru_remove(index, key);
                    lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
                }
            }
        }
    }
    #[cfg(not(any(target_os = "windows")))]
    pub async fn rdb_save(&self) -> Result<(), anyhow::Error> {
        let mut interval = interval(Duration::from_millis(10000));
        let file_path = "rcache.rdb";
        let config = config::standard();
        loop {
            interval.tick().await;
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(file_path.clone())?;
            let lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
            if let Ok(Fork::Child) = fork() {
                let _worker_guard = setup_logger();
                let database = lock.deref();
                let key_len = lock.data[0].len();
                let current_time = Instant::now();
                let mywriter = MyWriter(file);
                let res = bincode::encode_into_writer(database, mywriter, config.clone());
                if let Err(e) = res {
                    error!("{}", e);
                }
                let first_cost = current_time.elapsed();
                info!(
                    "Rdb file has been saved,keys count is {},encode time cost {}ms,total time cost {}ms",
                    key_len,
                    first_cost.as_millis(),
                    current_time.elapsed().as_millis()
                );
                println!(
                    "Rdb file has been saved,keys count is {},encode time cost {}ms,total time cost {}ms",
                    key_len,
                    first_cost.as_millis(),
                    current_time.elapsed().as_millis()
                );
                std::process::exit(0);
            }
            drop(lock);
        }
    }
    #[cfg(target_os = "windows")]
    pub async fn rdb_save(&self) -> Result<(), anyhow::Error> {
        let mut interval = interval(Duration::from_millis(10000));
        let file_path = "rcache.rdb";
        let config = config::standard();
        loop {
            interval.tick().await;
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true) // Create the file if it does not exist
                .open(file_path)?;
            let lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
            let database = lock.clone();
            drop(lock);
            let _worker_guard = setup_logger();

            let key_len = database.data[0].len();
            let current_time = Instant::now();
            let mywriter = MyWriter(file);

            let res = bincode::encode_into_writer(database, mywriter, config);
            if let Err(e) = res {
                error!("{}", e);
            }
            let first_cost = current_time.elapsed();
            info!(
                    "Rdb file has been saved,keys count is {},encode time cost {}ms,total time cost {}ms",
                    key_len,
                    first_cost.as_millis(),
                    current_time.elapsed().as_millis()
                );
            println!(
                    "Rdb file has been saved,keys count is {},encode time cost {}ms,total time cost {}ms",
                    key_len,
                    first_cost.as_millis(),
                    current_time.elapsed().as_millis()
                );
        }
    }
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]

pub struct Database {
    pub data: Vec<HashMap<Vec<u8>, Value>>,
    pub expire_map: Vec<HashMap<Vec<u8>, i64>>,
    pub node_info: NodeInfo,
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

impl Database {
    pub fn new() -> Self {
        let mut data_vec = vec![];
        let mut expire_map = vec![];
        let node_info = NodeInfo::new();
        for _i in 0..16 {
            data_vec.push(HashMap::new());
            expire_map.push(HashMap::new());
        }
        Database {
            data: data_vec,
            expire_map,
            node_info,
        }
    }
    pub fn get(&self, db_index: usize, key: Vec<u8>) -> Result<Option<&Value>, anyhow::Error> {
        let data = self
            .data
            .get(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get(&key.clone());
        Ok(data)
    }
    pub fn get_self(self) -> Self {
        self
    }
    pub fn insert(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        value: Value,
    ) -> Result<(), anyhow::Error> {
        self.data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .insert(key, value);
        Ok(())
    }
    pub fn lpush(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<usize, anyhow::Error> {
        let tt = Value::List(ValueList {
            data: VecDeque::new(),
        });
        let value_list = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .entry(key.clone())
            .or_insert_with(|| tt);
        value_list.lpush(value)
    }
    pub fn rpush(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<usize, anyhow::Error> {
        let tt = Value::List(ValueList {
            data: VecDeque::new(),
        });
        let value_list = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .entry(key.clone())
            .or_insert_with(|| tt);
        value_list.rpush(value)
    }
    pub fn lpop(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        count_option: Option<i64>,
    ) -> Result<Response, anyhow::Error> {
        let value_option = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key);
        if let Some(val) = value_option {
            let res = val.lpop(count_option)?;
            Ok(res)
        } else {
            Ok(Response::Nil)
        }
    }
    pub fn rpop(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        count_option: Option<i64>,
    ) -> Result<Response, anyhow::Error> {
        let value_option = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key);
        if let Some(val) = value_option {
            let res = val.rpop(count_option)?;
            Ok(res)
        } else {
            Ok(Response::Nil)
        }
    }
    pub fn lrange(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        start: i64,
        stop: i64,
    ) -> Result<Response, anyhow::Error> {
        let value_list_option = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key);
        match value_list_option {
            Some(r) => r.lrange(start, stop),
            None => Ok(Response::Array(vec![])),
        }
    }
    pub fn zadd(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        score: f64,
        member: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_sosrted_set = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .entry(key.clone())
            .or_insert_with(|| {
                Value::SortedSet(ValueSortedSet {
                    data: BTreeSet::new(),
                })
            });
        value_sosrted_set.zadd(member, score)
    }
    pub fn sadd(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_set = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .entry(key)
            .or_insert_with(|| {
                Value::Set(ValueSet {
                    data: HashSet::new(),
                })
            });
        value_set.sadd(value)
    }
    pub fn hset(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        field: Vec<u8>,

        value: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_set = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .entry(key.clone())
            .or_insert_with(|| {
                Value::Hash(ValueHash {
                    data: HashMap::new(),
                })
            });
        value_set.hset(field, value)
    }
    pub fn remove(&mut self, db_index: usize, key: Vec<u8>) -> Result<bool, anyhow::Error> {
        let removed = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .remove(&key);
        Ok(removed.is_some())
    }
    pub fn append(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<usize, anyhow::Error> {
        let map = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        match map.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let v = e.get_mut();
                if !v.is_string() {
                    return Err(anyhow::anyhow!("WrongTypeError"));
                }
                v.append(value)
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                let len = value.len();
                e.insert(Value::String(ValueString { data: value }));
                Ok(len)
            }
        }
    }
    pub fn strlen(&self, db_index: usize, key: Vec<u8>) -> Result<usize, anyhow::Error> {
        let value = self.get(db_index, key)?;
        match value {
            Some(v) => v.strlen(),
            None => Ok(0),
        }
    }
    pub fn srem(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_option = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key);
        match value_option {
            Some(v) => v.srem(value),
            None => Err(anyhow::anyhow!("no such key")),
        }
    }
    pub fn sismember(
        &self,
        db_index: usize,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.sismember(&value),
            None => Ok(false),
        }
    }
    pub fn scard(&self, db_index: usize, key: Vec<u8>) -> Result<usize, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.scard(),
            None => Ok(0),
        }
    }
    pub fn smembers(
        &self,
        db_index: usize,
        key: Vec<u8>,
    ) -> Result<Vec<Vec<u8>>, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.smembers(),
            None => Ok(vec![]),
        }
    }
    pub fn hget(
        &self,
        db_index: usize,
        key: Vec<u8>,
        field: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.hget(&field),
            None => Ok(None),
        }
    }
    pub fn hdel(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        field: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_option = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key);
        match value_option {
            Some(v) => v.hdel(&field),
            None => Ok(false),
        }
    }
    pub fn hexists(
        &self,
        db_index: usize,
        key: Vec<u8>,
        field: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.hexists(&field),
            None => Ok(false),
        }
    }
    pub fn hlen(&self, db_index: usize, key: Vec<u8>) -> Result<usize, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.hlen(),
            None => Ok(0),
        }
    }
    pub fn hgetall(
        &self,
        db_index: usize,
        key: Vec<u8>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.hgetall(),
            None => Ok(vec![]),
        }
    }
    pub fn zrange(
        &self,
        db_index: usize,
        key: Vec<u8>,
        start: i64,
        stop: i64,
    ) -> Result<Vec<Vec<u8>>, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.zrange(start, stop),
            None => Ok(vec![]),
        }
    }
    pub fn zrem(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        member: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let value_option = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key);
        match value_option {
            Some(v) => v.zrem(&member),
            None => Ok(false),
        }
    }
    pub fn zcard(&self, db_index: usize, key: Vec<u8>) -> Result<usize, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.zcard(),
            None => Ok(0),
        }
    }
    pub fn zscore(
        &self,
        db_index: usize,
        key: Vec<u8>,
        member: Vec<u8>,
    ) -> Result<Option<f64>, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.zscore(&member),
            None => Ok(None),
        }
    }
    pub fn llen(&self, db_index: usize, key: Vec<u8>) -> Result<usize, anyhow::Error> {
        let value_option = self.get(db_index, key)?;
        match value_option {
            Some(v) => v.llen(),
            None => Ok(0),
        }
    }
    /// Set expiration on a key. expire_at is a Unix timestamp in seconds.
    /// Returns true if set successfully, false if the key does not exist.
    pub fn set_expire(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        expire_at: i64,
    ) -> Result<bool, anyhow::Error> {
        let data_map = self
            .data
            .get(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        if !data_map.contains_key(&key) {
            return Ok(false);
        }
        let expire_map = self
            .expire_map
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        expire_map.insert(key, expire_at);
        Ok(true)
    }
    /// Get the remaining TTL of a key in seconds.
    /// Returns -2 if the key does not exist, -1 if no expiration is set,
    /// or the remaining seconds otherwise.
    pub fn get_ttl(&self, db_index: usize, key: Vec<u8>) -> Result<i64, anyhow::Error> {
        let data_map = self
            .data
            .get(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        if !data_map.contains_key(&key) {
            return Ok(-2);
        }
        let expire_map = self
            .expire_map
            .get(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        match expire_map.get(&key) {
            Some(&expire_at) => {
                let current_timestamp = Utc::now().timestamp();
                let remaining = expire_at - current_timestamp;
                Ok(remaining.max(0))
            }
            None => Ok(-1),
        }
    }
    /// Remove expiration from a key.
    /// Returns true if removed, false if the key does not exist or has no expiration.
    pub fn remove_expire(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
    ) -> Result<bool, anyhow::Error> {
        let data_map = self
            .data
            .get(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        if !data_map.contains_key(&key) {
            return Ok(false);
        }
        let expire_map = self
            .expire_map
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?;
        Ok(expire_map.remove(&key).is_some())
    }
}

async fn scan_expire(sender: mpsc::Sender<BackgroundEvent>) {
    let mut tick_stream = interval(Duration::from_millis(1000));
    loop {
        let _ = sender.send(BackgroundEvent::Nil).await;
        tick_stream.tick().await;
    }
}
