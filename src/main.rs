mod command;
mod database;
mod parser;
mod util;
mod vojo;
use crate::database::lib::Database;
use crate::parser::request::Request;
use crate::vojo::client::Client;

use crate::database::lib::TransferCommandData;
use anyhow::anyhow;
use log::info;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use tokio::time::Instant;

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();
    let addr = "0.0.0.0:6379";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address,{}", e))?;

    info!("Server listening on {}", addr);
    let (sender, receiver) = mpsc::channel(1);

    let mut database = Database::new();
    task::spawn(async move { database.handle_receiver(receiver).await });
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
}

async fn handle_connection(
    mut socket: TcpStream,
    sender: mpsc::Sender<TransferCommandData>,
) -> Result<(), anyhow::Error> {
    let mut buf = vec![0u8; 1024];
    let mut client = Client::new();
    loop {
        let cloned_client = client.clone();
        match socket.read(&mut buf).await {
            Ok(0) => {
                info!("Connection closed by client");
                break;
            }
            Ok(_) => {
                let (oneshot_sender, onesho_receiver) = oneshot::channel();
                let (parsed_command, _) = Request::parse_buf(&buf)?;

                let data = TransferCommandData {
                    parsed_command,
                    client: cloned_client,
                    sender: oneshot_sender,
                };
                sender.send(data).await?;
                let receive_data = onesho_receiver.await?;
                client.auth = receive_data.auth;
                client.dbindex = receive_data.dbindex;
                let data = receive_data.data;
                if let Err(e) = socket.write_all(&data).await {
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
