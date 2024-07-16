use crate::command::set_command::sadd;
use crate::command::sorted_set_command::zadd;
use crate::command::string_command::{get, set};
use crate::database::fs_writer::MyWriter;
use crate::parser::ping::ping;
use crate::parser::response::Response;
use crate::vojo::client::Client;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::BackgroundEvent;
use crate::vojo::value::Value;
use crate::vojo::value::{ValueSet, ValueSortedSet};
use crate::Request;
use std::sync::mpsc;

use std::borrow::Cow;
use std::collections::LinkedList;
use std::collections::{BTreeSet, HashMap};
use std::collections::{HashSet, VecDeque};
use std::ops::Deref;

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
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tracing_subscriber::fmt::format;
#[derive(Clone)]
pub struct DatabaseHolder {
    pub database_lock: Arc<Mutex<Database>>,
}
impl DatabaseHolder {
    pub fn expire_loop(&self) -> Result<(), anyhow::Error> {
        loop {
            std::thread::sleep(Duration::from_millis(200));

            let mut lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
            let current_time = Instant::now();

            for (index, map) in &mut lock.expire_map.iter_mut().enumerate() {
                // Collect keys that have expired
                let expired_keys: Vec<Vec<u8>> = map
                    .iter()
                    .filter(|(_, &time)| {
                        let time_duration = Duration::from_secs(time as u64);
                        let expiration_time = current_time.checked_sub(time_duration);
                        expiration_time.is_none() // If the expiration time is None, it's expired
                    })
                    .map(|(key, _)| key.clone())
                    .collect();

                // Remove expired keys from the hashmap
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
    pub fn rdb_save(&self) -> Result<(), anyhow::Error> {
        let file_path = "rcache.rdb";
        let config = config::standard();
        let now = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(10));

            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true) // Create the file if it does not exist
                .open(file_path)?;

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
                    "{:?},Rdb file has been saved,keys count is {},encode time cost {}ms,total time cost {}ms",
                    chrono::offset::Local::now(),
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
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true) // Create the file if it does not exist
                .open(file_path.clone())?;
            let lock = self.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
            let database = lock.clone();
            drop(lock);
            let _worker_guard = setup_logger();

            let key_len = database.data[0].len();
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
        }
    }
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]

pub struct Database {
    pub data: Vec<HashMap<Vec<u8>, Value>>,
    pub expire_map: Vec<HashMap<Vec<u8>, i64>>,
    pub node_info: NodeInfo,
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
}

async fn scan_expire(sender: mpsc::Sender<BackgroundEvent>) {
    let mut tick_stream = monoio::time::interval(Duration::from_millis(1000));
    loop {
        let _ = sender.send(BackgroundEvent::Nil);
        tick_stream.tick().await;
    }
}
