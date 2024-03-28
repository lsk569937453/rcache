use crate::command::parser::ParsedCommand;
use crate::command::ping::ping;
use crate::command::response::Response;
use crate::command::string_command::{get, set};
use tokio::sync::{mpsc, oneshot};

pub struct TransferData {
    pub parsed_command: ParsedCommand,
    pub sender: oneshot::Sender<Vec<u8>>,
}
pub struct RedisData {
    pub map: std::collections::HashMap<String, String>,
}
impl RedisData {
    pub async fn handle_receiver(&mut self, mut receiver: mpsc::Receiver<TransferData>) {
        while let Some(transfer_data) = receiver.recv().await {
            if let Err(e) = self.handle_receiver_with_error(transfer_data).await {
                info!("{}", e);
            }
        }
    }
    async fn handle_receiver_with_error(
        &mut self,
        transfer_data: TransferData,
    ) -> Result<(), anyhow::Error> {
        let parsed_command = transfer_data.parsed_command;

        let command_name = parsed_command.get_str(0)?;
        let data = match command_name {
            "ping" => ping(parsed_command),
            "set" => set(parsed_command, self),
            "get" => get(parsed_command, self),

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
