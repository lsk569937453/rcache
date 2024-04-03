mod command;
mod database;
mod parser;
mod util;
mod vojo;
use crate::database::lib::Database;
use crate::parser::request::Request;
use crate::vojo::client::Client;

use crate::command::string_command::{get, set};
use crate::database::lib::DatabaseHolder;
use crate::database::lib::TransferCommandData;
use anyhow::anyhow;

use crate::parser::ping::ping;
use crate::parser::response::Response;
use log::info;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task;
#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

pub struct Handler {
    pub connect: TcpStream,
    pub database_holder: DatabaseHolder,
}

impl Handler {
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        let mut buf = vec![0u8; 1024];

        let parsed_command = match self.connect.read(&mut buf).await {
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
        let database_holder = &mut self.database_holder;
        let command_name = parsed_command.get_str(0)?.to_uppercase();
        let result = match command_name.as_str() {
            "PING" => ping(parsed_command),
            "SET" => set(parsed_command, database_holder, db_index),
            "GET" => get(parsed_command, database_holder, db_index),
            _ => {
                info!("{}", command_name);
                Ok(Response::Nil)
            }
        };
        let data = match result {
            Ok(r) => r,
            Err(r) => Response::Error(r.to_string()),
        };
        self.connect.write_all(&data.as_bytes()).await?;
        Ok(())
    }
}
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();
    let addr = "0.0.0.0:6379";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address,{}", e))?;

    info!("Server listening on {}", addr);

    let database = DatabaseHolder {
        database_lock: Arc::new(Mutex::new(Database::new())),
    };
    loop {
        let (socket, _) = listener
            .accept()
            .await
            .expect("Failed to accept incoming connection");
        let cloned_database = database.clone();
        let handler = Handler {
            connect: socket,
            database_holder: cloned_database,
        };
        task::spawn(async move {
            if let Err(e) = handle_connection(handler).await {
                info!("{}", e);
            }
        });
    }
}

async fn handle_connection(mut handler: Handler) -> Result<(), anyhow::Error> {
    loop {
        handler.run().await?;
    }
}
