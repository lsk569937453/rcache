mod command;
use crate::command::parser::ParsedCommand;
use crate::command::ping::ping;
use crate::command::redis_data::RedisData;
use crate::command::redis_data::TransferData;
use crate::command::request::Request;
use crate::command::response::Response;
use anyhow::anyhow;
use log::info;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task;
#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    // Create a TCP listener bound to the specified address
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address"))?;

    println!("Server listening on {}", addr);
    let (sender, receiver) = mpsc::channel(1);
    let mut redis_data = RedisData {
        map: HashMap::new(),
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
    sender: mpsc::Sender<TransferData>,
) -> Result<(), anyhow::Error> {
    println!("New client connected");

    let mut buf = vec![0u8; 1024];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) => {
                println!("Connection closed by client");
                break;
            }
            Ok(n) => {
                let (oneshot_sender, onesho_receiver) = oneshot::channel();
                let (parsed_command, b) = Request::parse_buf(&buf)?;

                let data = TransferData {
                    parsed_command,
                    sender: oneshot_sender,
                };
                sender.send(data).await?;
                let receive_data = onesho_receiver.await?;

                if let Err(_) = socket.write_all(&receive_data).await {
                    println!("Error writing data to socket");
                    break;
                }
            }
            Err(err) => {
                println!("Error reading data from socket: {}", err);
                break;
            }
        }
    }
    Ok(())
}
