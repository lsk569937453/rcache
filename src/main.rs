mod command;
mod database;
mod parser;
mod util;
mod vojo;
use crate::database::lib::Database;

use crate::database::lib::DatabaseHolder;
use crate::parser::handler::Handler;

use clap::Parser;
use database::common::load_rdb;
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
    /// The port
    #[arg(default_value_t = 6379)]
    port: u32,
    /// The rdb path
    #[arg(short = 'r', long = "rdb_path", value_name = "rdb path")]
    rdb_path: Option<String>,
}

#[tokio::main]
async fn main() {
    if let Err(e) = main_with_error().await {
        println!("{e}");
    }
}

async fn main_with_error() -> Result<(), anyhow::Error> {
    let _worker_guard = setup_logger()?;
    let cli: Cli = Cli::parse();
    let port = cli.port;
    let addr = format!(r#"0.0.0.0:{port}"#);

    let database = if let Some(file_path) = cli.rdb_path {
        let database = load_rdb(file_path).await?;
        database
    } else {
        Database::new()
    };
    let database_holder = DatabaseHolder {
        database_lock: Arc::new(Mutex::new(database)),
    };

    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|_| anyhow!("Failed to bind to address,{}", addr))?;
    info!("Server listening on {}", addr);

    let _ = start_loop(database_holder.clone()).await;
    loop {
        let (socket, _) = listener
            .accept()
            .await
            .expect("Failed to accept incoming connection");
        let remote_addr = socket.peer_addr()?.to_string();

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
