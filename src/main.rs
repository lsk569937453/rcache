use std::sync::mpsc::Receiver;
mod command;
use crate::command::ping::ping;
use crate::command::request::Request;
use crate::command::response::Response;
use anyhow::anyhow;
use log::info;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task;
pub struct TransferData {
    data: Vec<u8>,
    sender: oneshot::Sender<Vec<u8>>,
}
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create a TCP listener bound to the specified address
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address"))?;

    println!("Server listening on {}", addr);
    let (sender, receiver) = mpsc::channel(1);
    task::spawn(async move { handle_receiver(receiver).await });
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

                let data = TransferData {
                    data: buf[..n].to_vec(),
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

async fn pre_send(buf: Vec<u8>) -> Result<(), anyhow::Error> {
    let (parsed_command, b) = Request::parse_buf(&buf)?;
    let command_name = parsed_command.get_str(0)?;
    let data = match command_name {
        "ping" => ping(parsed_command),
        _ => Ok(Response::Nil),
    };

    Ok(())
}
async fn handle_receiver(mut receiver: mpsc::Receiver<TransferData>) {
    while let Some(result) = receiver.recv().await {
        let sender = result.sender;
        let data = result.data;
        info!("data is :{}", String::from_utf8_lossy(&data));
        let _ = sender.send("t".as_bytes().to_vec());
    }
}
