mod command;
mod database;
mod parser;
mod util;
mod vojo;
use crate::database::lib::Database;
use crate::parser::request::Request;

use crate::database::lib::DatabaseHolder;
use crate::parser::handler::Handler;

use clap::Parser;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::net::TcpListener;
use tokio::task;

mod logger;
#[macro_use]
extern crate tracing;
#[macro_use]
extern crate anyhow;
use crate::logger::default_logger::setup_logger;
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Cli {
    /// The request url,like http://www.google.com
    #[arg(default_value_t = 6379)]
    port: u32,
}

#[tokio::main]

async fn main() -> Result<(), anyhow::Error> {
    let _worker_guard = setup_logger()?;
    let cli: Cli = Cli::parse();
    let port = cli.port;
    let addr = format!(r#"0.0.0.0:{port}"#);

    // Bind to the specified address and port
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|_| anyhow!("Failed to bind to address,{}", addr))?;
    info!("Server listening on {}", addr);

    // Create a new instance of our database
    let database_holder = DatabaseHolder {
        database_lock: Arc::new(Mutex::new(Database::new())),
    };

    // Spawn a new task that will run the database expiration loop
    let _ = start_loop(database_holder.clone()).await;
    loop {
        // Accept an incoming connection and get the remote address
        let (socket, _) = listener
            .accept()
            .await
            .expect("Failed to accept incoming connection");
        let remote_addr = socket.peer_addr()?.to_string();

        // Create a new handler and spawn a new task to handle the connection
        let cloned_database = database_holder.clone();
        let handler = Handler {
            connect: socket,
            database_holder: cloned_database,
        };
        task::spawn(async move {
            if let Err(e) = handle_connection(handler, remote_addr.clone()).await {
                info!("The error is {}", e);
            }
        });
    }
}
pub async fn start_loop(database_holder: DatabaseHolder) -> Result<(), anyhow::Error> {
    let cloned_database_holder1 = database_holder.clone();
    let cloned_database_holder2 = database_holder.clone();

    tokio::spawn(async move {
        if let Err(e) = cloned_database_holder1.expire_loop().await {
            error!("The error is {}", e);
        }
    });
    tokio::spawn(async move {
        if let Err(e) = cloned_database_holder2.rdb_save().await {
            error!("The error is {}", e);
        }
    });
    Ok(())
}
#[instrument(skip(handler))]
async fn handle_connection(
    mut handler: Handler,
    _remote_addr: String,
) -> Result<(), anyhow::Error> {
    loop {
        handler.run().await?;
    }
}
