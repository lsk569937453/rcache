use crate::command::string_command::{
    append, decr, decrby, get, getdel, getex, getrange, getset, incr, incrby, incrbyfloat, lcs,
    mget, mset, msetnx, set, setex,
};
use crate::parser::ping::ping;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::client::Client;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::BackgroundEvent;
use crate::vojo::value::Value;
use hashbrown::HashMap;

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
                            self.remove(index,k.to_vec());
                            self.expire_remove(index, k.to_vec());
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
