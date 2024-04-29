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
use tracing::metadata::LevelFilter;
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_appender::rolling;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;
use tracing_subscriber::{fmt, layer::SubscriberExt};
#[macro_use]
extern crate tracing;
#[macro_use]
extern crate anyhow;
fn setup_logger() -> Result<WorkerGuard, anyhow::Error> {
    let app_file = rolling::daily("./logs", "access.log");
    let (non_blocking_appender, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(10)
        .finish(app_file);
    let file_layer = tracing_subscriber::fmt::Layer::new()
        .with_target(true)
        .with_ansi(false)
        .with_writer(non_blocking_appender)
        .with_filter(tracing_subscriber::filter::LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();
    Ok(guard)
}
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let _worker_guard = setup_logger()?;

    let addr = "0.0.0.0:6379";
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to address,{}", e))?;

    info!("Server listening on {}", addr);

    let database = DatabaseHolder {
        database_lock: Arc::new(Mutex::new(Database::new())),
    };
    loop {
        let (socket, socket_addr) = listener
            .accept()
            .await
            .expect("Failed to accept incoming connection");
        let remote_addr = socket.peer_addr()?.to_string();
        let cloned_database = database.clone();
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
#[instrument(skip(handler))]
async fn handle_connection(
    mut handler: Handler,

    _remote_addr: String,
) -> Result<(), anyhow::Error> {
    loop {
        handler.run().await?;
    }
}
