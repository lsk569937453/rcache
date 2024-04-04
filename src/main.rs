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
use crate::parser::handler::Handler;
use crate::parser::ping::ping;
use crate::parser::response::Response;
use anyhow::anyhow;
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
