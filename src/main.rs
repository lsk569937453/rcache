mod cluster;
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
use crate::cluster::gossip;
use crate::cluster::state::{ClusterHolder, ClusterState};
use crate::logger::default_logger::setup_logger;
use chrono::Utc;

#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Cli {
    /// The port
    #[arg(default_value_t = 6370)]
    port: u32,
    /// The rdb path
    #[arg(short = 'r', long = "rdb_path", value_name = "rdb path")]
    rdb_path: Option<String>,
    /// Enable cluster mode
    #[arg(long = "cluster-enabled")]
    cluster_enabled: bool,
    /// Cluster node timeout in milliseconds
    #[arg(long = "cluster-node-timeout", default_value_t = 15000)]
    cluster_node_timeout: u64,
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

    // Initialize cluster state if enabled
    let cluster_holder = if cli.cluster_enabled {
        let self_addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
        let timestamp = Utc::now().timestamp();
        let state = ClusterState::new(self_addr, timestamp);
        let holder = ClusterHolder::new(state);
        info!(
            "Cluster mode enabled, node ID: {}",
            {
                let s = holder.state.read().await;
                s.my_id().to_string()
            }
        );
        Some(holder)
    } else {
        None
    };

    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|_| anyhow!("Failed to bind to address,{}", addr))?;
    info!("Server listening on {}", addr);

    let _ = start_loop(database_holder.clone()).await;

    // Start gossip tasks if cluster mode is enabled
    if let Some(ref ch) = cluster_holder {
        // Start cluster bus listener
        let bus_holder = ch.clone();
        let bus_port = port as u16 + 10000;
        tokio::spawn(async move {
            gossip::start_gossip_listener(bus_holder, bus_port).await;
        });

        // Start gossip loop
        let gossip_holder = ch.clone();
        tokio::spawn(async move {
            gossip::start_gossip_loop(gossip_holder).await;
        });
    }

    loop {
        let (socket, _) = listener.accept().await?;
        let remote_addr = socket.peer_addr()?.to_string();

        let cloned_database = database_holder.clone();
        let cloned_cluster = cluster_holder.clone();
        let handler = Handler {
            connect: socket,
            database_holder: cloned_database,
            cluster_holder: cloned_cluster,
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
