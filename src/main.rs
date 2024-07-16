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
use database::common::load_rdb;
use monoio::{
    io::{AsyncReadRent, AsyncWriteRentExt},
    net::{TcpListener, TcpStream},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

mod logger;
#[macro_use]
extern crate tracing;
#[macro_use]
extern crate anyhow;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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

fn main() -> Result<(), anyhow::Error> {
    let cli: Cli = Cli::parse();

    let database = if let Some(file_path) = cli.rdb_path {
        let database = load_rdb(file_path)?;
        database
    } else {
        Database::new()
    };

    // Create a new instance of our database
    let database_holder = DatabaseHolder {
        database_lock: Arc::new(Mutex::new(database)),
    };
    let _ = start_loop(database_holder.clone());
    let port = cli.port;
    let addr = format!(r#"0.0.0.0:{port}"#);
    std::thread::scope(|s| {
        let addr_clone = addr.clone();
        let database_clone = database_holder.clone();
        for i in 0..4 {
            println!("thread is {}", i);
            let addr_clone1 = addr_clone.clone();
            let database_clone1 = database_clone.clone();
            s.spawn(move || {
                let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
                    .with_entries(256)
                    .enable_timer()
                    .build()
                    .unwrap();
                rt.block_on(async {
                    if let Err(e) = main_with_error(addr_clone1, database_clone1).await {
                        println!("{}", e);
                    }
                });
            });
        }
    });
    Ok(())
}

async fn main_with_error(
    addr: String,
    database_holder: DatabaseHolder,
) -> Result<(), anyhow::Error> {
    let _worker_guard = setup_logger()?;

    // Bind to the specified address and port
    let listener =
        TcpListener::bind(&addr).map_err(|_| anyhow!("Failed to bind to address,{}", addr))?;
    info!("Server listening on {}", addr);

    // Spawn a new task that will run the database expiration loop
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
        monoio::spawn(async move {
            if let Err(e) = handle_connection(handler, remote_addr.clone()).await {
                info!("The error is {}", e);
            }
        });
    }
}
pub fn start_loop(database_holder: DatabaseHolder) -> Result<(), anyhow::Error> {
    let cloned_database_holder1 = database_holder.clone();
    let cloned_database_holder2 = database_holder.clone();

    std::thread::spawn(move || {
        if let Err(e) = cloned_database_holder1.expire_loop() {
            error!("The error is {}", e);
        }
    });

    std::thread::spawn(move || {
        if let Err(e) = cloned_database_holder2.rdb_save() {
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
