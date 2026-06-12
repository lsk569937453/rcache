#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod cluster;
mod command;
mod config;
mod database;
mod parser;
mod util;
mod vojo;
use crate::database::lib::Database;

use crate::database::lib::{DatabaseHolder, LruState};
use crate::parser::handler::Handler;

use clap::Parser;
use config::AppConfig;
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
    /// Config file path (YAML)
    #[arg(short = 'f', long = "config", value_name = "FILE")]
    config: String,
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
    let app_config = AppConfig::load(&cli.config)?;
    info!("Loaded config from: {}", cli.config);

    let port = app_config.server.port;
    let addr = format!(r#"0.0.0.0:{port}"#);

    // Load database: RDB first, then AOF on top (AOF is more complete)
    let mut database = if let Some(ref rdb_path) = app_config.persistence.rdb_path {
        let path = rdb_path.clone();
        if std::path::Path::new(&path).exists() {
            load_rdb(path).await?
        } else {
            Database::new()
        }
    } else {
        Database::new()
    };

    // Initialize LRU trackers and memory tracker
    let max_memory = app_config.max_memory_bytes();
    let mut lru_trackers = vec![];
    for _ in 0..16 {
        lru_trackers.push(crate::database::lru::LruTracker::new());
    }
    let lru_state = LruState {
        lru_trackers,
        memory_tracker: crate::database::lru::MemoryTracker::new(max_memory),
    };

    // Load AOF if enabled and file exists
    let aof_writer = if app_config.persistence.aof_enabled {
        let aof_path = app_config.aof_path_buf();
        if aof_path.exists() {
            info!("Loading AOF file: {:?}", aof_path);
            crate::database::aof::load_aof(&aof_path, &mut database)?;
            info!("AOF loaded successfully");
        }
        let writer = crate::database::aof::AofWriter::new(
            app_config.persistence.aof_path.clone(),
            crate::database::aof::parse_sync_policy(&app_config.persistence.aof_sync_policy),
        )?;
        Some(Arc::new(writer))
    } else {
        None
    };

    let database_holder = DatabaseHolder {
        database_lock: Arc::new(Mutex::new(database)),
        aof_writer,
        lru_state: Arc::new(Mutex::new(lru_state)),
    };

    // Initialize LRU tracking for existing keys (from RDB/AOF)
    database_holder.init_lru_from_database();

    // Initialize cluster state if enabled
    let cluster_holder = if app_config.server.cluster_enabled {
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
