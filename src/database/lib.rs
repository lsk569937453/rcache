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
use std::collections::HashSet;
use std::collections::LinkedList;
use std::collections::{BTreeSet, HashMap};

use crate::vojo::value::ValueList;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::interval;
#[derive(Clone)]
pub struct DatabaseHolder {
    pub database_lock: Arc<Mutex<Database>>,
}

pub struct Database {
    pub data: Vec<HashMap<Vec<u8>, Value>>,
    pub expire_map: Vec<HashMap<Vec<u8>, i64>>,
}

impl Database {
    pub fn new() -> Self {
        let mut data_vec = vec![];
        let mut expire_map = vec![];

        for _i in 0..16 {
            data_vec.push(HashMap::new());
            expire_map.push(HashMap::new());
        }
        Database {
            data: data_vec,
            expire_map,
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
            data: LinkedList::new(),
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
            data: LinkedList::new(),
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
    // pub async fn handle_receiver_with_error(
    //     &mut self,
    //     parsed_command: ParsedCommand,
    // ) -> Result<(), anyhow::Error> {
    //     let db_index = 0;
    //     let command_name = parsed_command.get_str(0)?.to_uppercase();
    //     let result = match command_name.as_str() {
    //         "PING" => ping(parsed_command),
    //         "SET" => set(parsed_command, self, db_index),

    //         "GET" => get(parsed_command, self, db_index),

    //         _ => {
    //             info!("{}", command_name);
    //             Ok(Response::Nil)
    //         }
    //     };
    //     let data = match result {
    //         Ok(r) => r,
    //         Err(r) => Response::Error(r.to_string()),
    //     };

    //     Ok(())
    // }
}

async fn scan_expire(sender: mpsc::Sender<BackgroundEvent>) {
    let mut tick_stream = interval(Duration::from_millis(1000));
    loop {
        let _ = sender.send(BackgroundEvent::Nil).await;
        tick_stream.tick().await;
    }
}
