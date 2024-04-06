use crate::command::set_command::sadd;
use crate::command::sorted_set_command::zadd;
use crate::command::string_command::{get, set};
use crate::parser::ping::ping;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::client::Client;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::{BackgroundEvent, ValueList};
use crate::vojo::value::{Value, ValueHash};
use crate::vojo::value::{ValueSet, ValueSortedSet};
use crate::Request;
use std::collections::HashSet;
use std::collections::LinkedList;
use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
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
impl DatabaseHolder {
    pub async fn handle_receiver_with_error(
        &mut self,
        mut tcpstream: TcpStream,
    ) -> Result<(), anyhow::Error> {
        let mut buf = vec![0u8; 1024];

        let parsed_command = match tcpstream.read(&mut buf).await {
            Ok(0) => {
                info!("Connection closed by client");
                return Err(anyhow!(""));
            }
            Ok(_) => {
                let (parsed_command, _) = Request::parse_buf(&buf)?;
                parsed_command
            }
            Err(err) => {
                error!("Error reading data from socket: {}", err);
                return Err(anyhow!(""));
            }
        };
        let db_index = 0;
        let command_name = parsed_command.get_str(0)?.to_uppercase();
        let result = match command_name.as_str() {
            "PING" => ping(parsed_command),
            "SET" => set(parsed_command, self, db_index),
            "GET" => get(parsed_command, self, db_index),
            "SADD" => sadd(parsed_command, self, db_index),
            "ZADD" => zadd(parsed_command, self, db_index),
            _ => {
                info!("{}", command_name);
                Ok(Response::Nil)
            }
        };
        let data = match result {
            Ok(r) => r,
            Err(r) => Response::Error(r.to_string()),
        };
        tcpstream.write_all(&data.as_bytes()).await?;
        Ok(())
    }
}
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
