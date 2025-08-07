use crate::database::fs_writer::MyWriter;
use crate::parser::response::Response;

use crate::vojo::value::BackgroundEvent;
use crate::vojo::value::Value;
use crate::vojo::value::{ValueSet, ValueSortedSet};
use chrono::TimeZone;
use chrono::Utc;
use std::time::SystemTime;

use std::collections::{BTreeSet, HashMap};
use std::collections::{HashSet, VecDeque};

use super::info::NodeInfo;
use crate::logger::default_logger::setup_logger;
use crate::vojo::value::ValueHash;
use crate::vojo::value::ValueList;
use bincode::{config, Decode, Encode};
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
use std::time::UNIX_EPOCH;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio::time::Instant;

#[derive(Clone)]
pub struct DatabaseHolder {
    pub database_lock: Arc<Mutex<Database>>,
}
impl DatabaseHolder {
    pub async fn expire_loop(&self) -> Result<(), anyhow::Error> {
        let mut interval = interval(Duration::from_millis(200));
        loop {
            interval.tick().await;

            let mut lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
            let current_time = Instant::now();

            for (index, map) in &mut lock.expire_map.iter_mut().enumerate() {
                let expired_keys: Vec<Vec<u8>> = map
                    .iter()
                    .filter(|(_, &time)| {
                        let time_duration = Duration::from_secs(time as u64);
                        let expiration_time = current_time.checked_sub(time_duration);
                        expiration_time.is_none() // If the expiration time is None, it's expired
                    })
                    .map(|(key, _)| key.clone())
                    .collect();

                for key in expired_keys {
                    debug!(
                        "the key |{:?}| in slot {} has been removed",
                        key.clone(),
                        index
                    );
                    map.remove(&key);
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
            debug!(
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
    pub fn get(&mut self, db_index: usize, key: Vec<u8>) -> Result<Option<&Value>, anyhow::Error> {
        if self.is_expired(db_index, &key)? {
            return Ok(None);
        }
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
        self.expire_map
            .get_mut(db_index)
            .ok_or_else(|| anyhow!("Invalid DB index"))?
            .remove(&key);
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
        self.is_expired(db_index, &key)?;
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
    fn is_expired(&mut self, db_index: usize, key: &[u8]) -> Result<bool, anyhow::Error> {
        let expire_time = self
            .expire_map
            .get(db_index)
            .ok_or_else(|| anyhow!("Invalid DB index"))?
            .get(key);

        if let Some(expiry) = expire_time {
            if Utc::now().timestamp() > *expiry {
                self.data
                    .get_mut(db_index)
                    .ok_or(anyhow!("Invalid DB index"))?
                    .remove(key);
                self.expire_map
                    .get_mut(db_index)
                    .ok_or(anyhow!("Invalid DB index"))?
                    .remove(key);
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn remove(&mut self, db_index: usize, key: &[u8]) -> Result<Option<Value>, anyhow::Error> {
        if self.is_expired(db_index, key)? {
            return Ok(None);
        }

        self.expire_map
            .get_mut(db_index)
            .ok_or_else(|| anyhow!("Invalid DB index"))?
            .remove(key);

        let value = self
            .data
            .get_mut(db_index)
            .ok_or_else(|| anyhow!("Invalid DB index"))?
            .remove(key);

        Ok(value)
    }
    pub fn contains_key(&mut self, db_index: usize, key: &[u8]) -> Result<bool, anyhow::Error> {
        if self.is_expired(db_index, key)? {
            return Ok(false);
        }
        Ok(self.data[db_index].contains_key(key))
    }

    pub fn keys(&self, db_index: usize) -> Result<Vec<Vec<u8>>, anyhow::Error> {
        let keys = self
            .data
            .get(db_index)
            .ok_or_else(|| anyhow!("Invalid DB index"))?
            .keys()
            .cloned()
            .collect();
        Ok(keys)
    }

    pub fn set_expire(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        seconds: u64,
    ) -> Result<bool, anyhow::Error> {
        if !self.data[db_index].contains_key(&key) {
            return Ok(false); // 键不存在，返回 0 (false)
        }

        let future_time = SystemTime::now() + Duration::from_secs(seconds);
        let expire_at = future_time.duration_since(UNIX_EPOCH)?.as_millis() as i64;
        self.expire_map
            .get_mut(db_index)
            .ok_or_else(|| anyhow!("Invalid DB index"))?
            .insert(key, expire_at);

        Ok(true)
    }

    pub fn get_ttl(&mut self, db_index: usize, key: &[u8]) -> Result<i64, anyhow::Error> {
        if !self.contains_key(db_index, key)? {
            return Ok(-2);
        }

        match self.expire_map[db_index].get(key) {
            Some(expire_at) => {
                let future_time = Utc.timestamp_millis_opt(*expire_at).unwrap();
                let now = Utc::now();
                let duration = future_time.signed_duration_since(now);
                Ok(duration.as_seconds_f32() as i64)
            }
            None => Ok(-1),
        }
    }
}

async fn scan_expire(sender: mpsc::Sender<BackgroundEvent>) {
    let mut tick_stream = interval(Duration::from_millis(1000));
    loop {
        let _ = sender.send(BackgroundEvent::Nil).await;
        tick_stream.tick().await;
    }
}
