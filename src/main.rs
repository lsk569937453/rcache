mod command;
mod database;
mod parser;
mod util;
mod vojo;
use crate::database::lib::Database;
use crate::parser::request::Request;

use crate::database::lib::DatabaseHolder;
use crate::parser::handler::Handler;

use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
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
                info!("The error is {}", e);
            }
        });
    }
}

async fn handle_connection(mut handler: Handler) -> Result<(), anyhow::Error> {
    loop {
        handler.run().await?;
    }
}
