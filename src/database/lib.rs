use crate::command::hash_command::hset;
use crate::command::list_command::{lpop, lpush, lrange, rpop, rpush};
use crate::command::set_command::sadd;
use crate::command::sorted_set_command::zadd;
use crate::command::string_command::{
    append, decr, decrby, get, getdel, getex, getrange, getset, incr, incrby, incrbyfloat, lcs,
    mget, mset, msetnx, set, setex,
};
use crate::parser::ping::ping;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::client::Client;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::{BackgroundEvent, ValueList};
use crate::vojo::value::{Value, ValueHash};
use crate::vojo::value::{ValueSet, ValueSortedSet};
use std::collections::HashSet;
use std::collections::LinkedList;
use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::interval;
pub struct Database {
    pub data: Vec<HashMap<Vec<u8>, Value>>,
    pub expire_map: Vec<HashMap<Vec<u8>, i64>>,
}
pub struct TransferCommandData {
    pub parsed_command: ParsedCommand,
    pub client: Client,
    pub sender: oneshot::Sender<Client>,
}
impl Database {
    pub fn new() -> Self {
        let mut data_vec = vec![];
        let mut expire_map = vec![];

        for i in 0..16 {
            data_vec.push(HashMap::new());
            expire_map.push(HashMap::new());
        }
        Database {
            data: data_vec,
            expire_map,
        }
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
    pub fn expire_insert(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
        expire_time: i64,
    ) -> Result<(), anyhow::Error> {
        self.expire_map
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .insert(key, expire_time);
        Ok(())
    }
    pub fn get(&self, db_index: usize, key: Vec<u8>) -> Result<Option<&Value>, anyhow::Error> {
        let data = self
            .data
            .get(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get(&key.clone());
        Ok(data)
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
    pub fn get_mut(
        &mut self,
        db_index: usize,
        key: Vec<u8>,
    ) -> Result<Option<&mut Value>, anyhow::Error> {
        let data = self
            .data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .get_mut(&key.clone());
        Ok(data)
    }

    pub fn remove(&mut self, db_index: usize, key: Vec<u8>) -> Result<(), anyhow::Error> {
        self.data
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .remove(&key.clone());
        Ok(())
    }
    pub fn expire_remove(&mut self, db_index: usize, key: Vec<u8>) -> Result<(), anyhow::Error> {
        self.expire_map
            .get_mut(db_index)
            .ok_or(anyhow::anyhow!("can not find db index-{}", db_index))?
            .remove(&key);
        Ok(())
    }
    pub async fn handle_receiver(
        &mut self,
        mut command_receiver: mpsc::Receiver<TransferCommandData>,
    ) {
        let (sender, mut receiver) = mpsc::channel(1);
        tokio::spawn(async move { scan_expire(sender).await });
        loop {
            tokio::select! {
                Some(transfer_data) = command_receiver.recv()=>{
                    if let Err(e) = self.handle_receiver_with_error(transfer_data).await {
                        info!("{}", e);
                    }
                }
                Some(_)=receiver.recv()=>{
                    let expire_map=self.expire_map.clone();

                    for (index,item) in expire_map.iter().enumerate() {
                        for (k, v) in item{
                        if v.clone() < mstime() {
                            let _=self.remove(index,k.to_vec());
                            let _=self.expire_remove(index, k.to_vec());
                        }}
                    }

                }
            }
        }
    }

    async fn handle_receiver_with_error(
        &mut self,
        transfer_data: TransferCommandData,
    ) -> Result<(), anyhow::Error> {
        let parsed_command = transfer_data.parsed_command;
        let mut client = transfer_data.client;
        let db_index = client.dbindex;

        let command_name = parsed_command.get_str(0)?.to_uppercase();
        let result = match command_name.as_str() {
            "PING" => ping(parsed_command),
            "SET" => set(parsed_command, self, db_index),
            "APPEND" => append(parsed_command, self, db_index),
            "DECR" => decr(parsed_command, self, db_index),
            "DECRBY" => decrby(parsed_command, self, db_index),
            "GET" => get(parsed_command, self, db_index),
            "GETDEL" => getdel(parsed_command, self, db_index),
            "GETEX" => getex(parsed_command, self, db_index),
            "GETRANGE" => getrange(parsed_command, self, db_index),
            "GETSET" => getset(parsed_command, self, db_index),
            "INCR" => incr(parsed_command, self, db_index),
            "INCRBY" => incrby(parsed_command, self, db_index),
            "INCRBYFLOAT" => incrbyfloat(parsed_command, self, db_index),
            "LCS" => lcs(parsed_command, self, db_index),
            "MGET" => mget(parsed_command, self, db_index),
            "MSET" => mset(parsed_command, self, db_index),
            "MSETNX" => msetnx(parsed_command, self, db_index),
            "LPUSH" => lpush(parsed_command, self, db_index),
            "RPUSH" => rpush(parsed_command, self, db_index),
            "LPOP" => lpop(parsed_command, self, db_index),
            "RPOP" => rpop(parsed_command, self, db_index),
            "SADD" => sadd(parsed_command, self, db_index),
            "HSET" => hset(parsed_command, self, db_index),
            "ZADD" => zadd(parsed_command, self, db_index),
            "LRANGE" => lrange(parsed_command, self, db_index),

            "SETEX" => setex(parsed_command, self, db_index),
            _ => {
                info!("{}", command_name);
                Ok(Response::Nil)
            }
        };
        let data = match result {
            Ok(r) => r,
            Err(r) => Response::Error(r.to_string()),
        };
        client.data = data.as_bytes();
        tokio::spawn(async move {
            let _ = transfer_data.sender.send(client);
        });
        Ok(())
    }
}

async fn scan_expire(sender: mpsc::Sender<BackgroundEvent>) {
    let mut tick_stream = interval(Duration::from_millis(1000));
    loop {
        let _ = sender.send(BackgroundEvent::Nil).await;
        tick_stream.tick().await;
    }
}