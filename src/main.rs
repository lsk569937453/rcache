mod command;
mod parser;
mod util;
mod vojo;
use crate::parser::ping::ping;
use crate::parser::request::Request;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::redis_data::RedisData;
use crate::vojo::redis_data::TransferCommandData;
use anyhow::anyhow;
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task;
#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    ::std::env::set_var("RUST_LOG", "info");

    env_logger::init();

    // Create a TCP listener bound to the specified address
    let addr = "0.0.0.0:6379";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address"))?;

    info!("Server listening on {}", addr);
    let (sender, receiver) = mpsc::channel(1);
    let mut redis_data = RedisData {
        string_value: HashMap::new(),
        expire_map: HashMap::new(),
    };
    task::spawn(async move { redis_data.handle_receiver(receiver).await });
    loop {
        let (socket, _) = listener
            .accept()
            .await
            .expect("Failed to accept incoming connection");
        let cloned_sender = sender.clone();
        task::spawn(async move {
            if let Err(e) = handle_connection(socket, cloned_sender).await {
                info!("{}", e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    mut socket: TcpStream,
    sender: mpsc::Sender<TransferCommandData>,
) -> Result<(), anyhow::Error> {
    info!("New client connected");

    let mut buf = vec![0u8; 1024];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) => {
                error!("Connection closed by client");
                break;
            }
            Ok(_) => {
                let (oneshot_sender, onesho_receiver) = oneshot::channel();
                let (parsed_command, _) = Request::parse_buf(&buf)?;

                let data = TransferCommandData {
                    parsed_command,
                    sender: oneshot_sender,
                };
                sender.send(data).await?;
                let receive_data = onesho_receiver.await?;

                if let Err(e) = socket.write_all(&receive_data).await {
                    error!("Error writing data to socket,{}", e);
                    break;
                }
            }
            Err(err) => {
                error!("Error reading data from socket: {}", err);
                break;
            }
        }
    }
    Ok(())
}
