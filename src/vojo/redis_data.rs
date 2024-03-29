use crate::command::string_command::{get, set, setex};
use crate::parser::ping::ping;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::interval;
pub struct TransferCommandData {
    pub parsed_command: ParsedCommand,
    pub sender: oneshot::Sender<Vec<u8>>,
}

pub enum BackgroundEvent {
    Nil,
}
pub struct RedisData {
    pub string_value: HashMap<Vec<u8>, Vec<u8>>,
    pub expire_map: HashMap<Vec<u8>, i64>,
}
impl RedisData {
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
                Some(data)=receiver.recv()=>{
                    let expire_map=self.expire_map.clone();

                    for (k,v) in expire_map.iter() {
                        if v < &mstime() {
                            self.string_value.remove(k);
                            self.expire_map.remove(k);
                        }
                    }

                    info!("{}",expire_map.len());
                }
            }
        }
    }

    async fn handle_receiver_with_error(
        &mut self,
        transfer_data: TransferCommandData,
    ) -> Result<(), anyhow::Error> {
        let parsed_command = transfer_data.parsed_command;

        let command_name = parsed_command.get_str(0)?;
        let data = match command_name {
            "ping" => ping(parsed_command),
            "SET" => set(parsed_command, self),
            "GET" => get(parsed_command, self),
            "setex" => setex(parsed_command, self),
            _ => {
                info!("{}", command_name);
                Ok(Response::Nil)
            }
        }?;
        tokio::spawn(async move {
            let _ = transfer_data.sender.send(data.as_bytes());
        });
        Ok(())
    }
}
async fn scan_expire(sender: mpsc::Sender<BackgroundEvent>) {
    let mut tick_stream = interval(Duration::from_millis(1000));
    loop {
        sender.send(BackgroundEvent::Nil).await;
        tick_stream.tick().await;
    }
}
