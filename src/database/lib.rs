use crate::command::set_command::sadd;
use crate::command::sorted_set_command::zadd;
use crate::command::string_command::{get, set};
use crate::parser::ping::ping;
use crate::parser::response::Response;

use crate::vojo::client::Client;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::BackgroundEvent;
use crate::vojo::value::Value;
use crate::vojo::value::{ValueSet, ValueSortedSet};
use crate::Request;
use std::borrow::Cow;
use std::collections::LinkedList;
use std::collections::{BTreeSet, HashMap};
use std::collections::{HashSet, VecDeque};

use super::info::NodeInfo;
use crate::vojo::value::ValueHash;
use crate::vojo::value::ValueList;
use arc_swap::ArcSwap;
use bincode::{config, Decode, Encode};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use std::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::interval;
use tokio::time::Instant;
use tracing_subscriber::fmt::format;
#[derive(Clone)]
pub struct DatabaseHolder {
    pub database_lock: Arc<Mutex<Database>>,
}
impl DatabaseHolder {
    pub async fn expire_loop(&self) -> Result<(), anyhow::Error> {
        let mut interval = interval(Duration::from_millis(200));
        loop {
            interval.tick().await;

            let mut lock = self.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
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
    pub async fn rdb_save(&self) -> Result<(), anyhow::Error> {
        let mut interval = interval(Duration::from_millis(10000));
        let file_path = format!("{}.rdb", "test");
        let config = config::standard();
        loop {
            interval.tick().await;
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true) // Create the file if it does not exist
                .open(file_path.clone())
                .await?;
            let lock = self.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
            let key_len = lock.data[0].len();
            let data_base=lock.clone();
            drop(lock);

            let current_time = Instant::now();
            let encoded: Vec<u8> ={
                bincode::encode_to_vec(data_base, config.clone()).unwrap()
            };
            let first_cost=current_time.elapsed();
            let _ = file.write_all(&encoded).await;
            info!(
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
    let mut tick_stream = interval(Duration::from_millis(1000));
    loop {
        let _ = sender.send(BackgroundEvent::Nil).await;
        tick_stream.tick().await;
    }
}
