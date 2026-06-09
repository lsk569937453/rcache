use crate::cluster::commands::cluster_command;
use crate::cluster::redirect::{check_cluster_redirect, key_index_for_command};
use crate::cluster::state::ClusterHolder;
use crate::command::client_command::client_command;
use crate::command::connection_command::{auth, hello, info, select_db};
use crate::command::hash_command::{hdel, hget, hgetall, hexists, hlen, hset};
use crate::command::list_command::{llen, lpop, lpush, lrange, rpop, rpush};
use crate::command::set_command::{sadd, scard, sismember, smembers, srem};
use crate::command::sorted_set_command::{zadd, zcard, zrange, zrem, zscore};
use crate::command::string_command::{
    append, dbsize, decr, decrby, del, exists, expire, get, getdel, getrange, getset, incr,
    incrby, incrbyfloat, keys, mget, mset, msetnx, set, strlen, ttl, type_cmd,
};
use crate::database::lib::DatabaseHolder;
use crate::parser::ping::ping;
use crate::parser::request::Request;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Check if a command is a write command (should be logged to AOF)
fn is_write_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "SET"
            | "APPEND"
            | "INCR"
            | "DECR"
            | "INCRBY"
            | "DECRBY"
            | "INCRBYFLOAT"
            | "GETSET"
            | "GETDEL"
            | "MSET"
            | "MSETNX"
            | "DEL"
            | "EXPIRE"
            | "LPUSH"
            | "RPUSH"
            | "LPOP"
            | "RPOP"
            | "SADD"
            | "SREM"
            | "HSET"
            | "HDEL"
            | "ZADD"
            | "ZREM"
    )
}

pub struct Handler {
    pub connect: TcpStream,
    pub database_holder: DatabaseHolder,
    pub cluster_holder: Option<ClusterHolder>,
}

impl Handler {
    /// Process a single command and return its response.
    async fn process_command(
        parsed_command: ParsedCommand,
        db_index: usize,
        database_holder: &mut DatabaseHolder,
        cluster_holder: &mut Option<ClusterHolder>,
    ) -> Response {
        let command_name = match parsed_command.get_str(0) {
            Ok(name) => name.to_uppercase(),
            Err(_) => return Response::Error("ERR invalid command".to_string()),
        };

        info!("Received command: {}", command_name);

        let result = match command_name.as_str() {
            "PING" => ping(parsed_command),
            // Connection commands
            "HELLO" => hello(parsed_command, cluster_holder),
            "AUTH" => auth(parsed_command),
            "SELECT" => select_db(parsed_command),
            "CLIENT" => client_command(parsed_command),
            "INFO" => info(parsed_command, database_holder, db_index, cluster_holder),
            // String commands
            "SET" => set(parsed_command, database_holder, db_index),
            "GET" => get(parsed_command, database_holder, db_index),
            "APPEND" => append(parsed_command, database_holder, db_index),
            "INCR" => incr(parsed_command, database_holder, db_index),
            "DECR" => decr(parsed_command, database_holder, db_index),
            "INCRBY" => incrby(parsed_command, database_holder, db_index),
            "DECRBY" => decrby(parsed_command, database_holder, db_index),
            "INCRBYFLOAT" => incrbyfloat(parsed_command, database_holder, db_index),
            "STRLEN" => strlen(parsed_command, database_holder, db_index),
            "GETRANGE" => getrange(parsed_command, database_holder, db_index),
            "GETSET" => getset(parsed_command, database_holder, db_index),
            "GETDEL" => getdel(parsed_command, database_holder, db_index),
            "MGET" => mget(parsed_command, database_holder, db_index),
            "MSET" => mset(parsed_command, database_holder, db_index),
            "MSETNX" => msetnx(parsed_command, database_holder, db_index),
            // Key commands
            "DEL" => del(parsed_command, database_holder, db_index),
            "EXISTS" => exists(parsed_command, database_holder, db_index),
            "TYPE" => type_cmd(parsed_command, database_holder, db_index),
            "DBSIZE" => dbsize(parsed_command, database_holder, db_index),
            "KEYS" => keys(parsed_command, database_holder, db_index),
            "EXPIRE" => expire(parsed_command, database_holder, db_index),
            "TTL" => ttl(parsed_command, database_holder, db_index),
            // List commands
            "LPUSH" => lpush(parsed_command, database_holder, db_index),
            "RPUSH" => rpush(parsed_command, database_holder, db_index),
            "LPOP" => lpop(parsed_command, database_holder, db_index),
            "RPOP" => rpop(parsed_command, database_holder, db_index),
            "LRANGE" => lrange(parsed_command, database_holder, db_index),
            "LLEN" => llen(parsed_command, database_holder, db_index),
            // Set commands
            "SADD" => sadd(parsed_command, database_holder, db_index),
            "SREM" => srem(parsed_command, database_holder, db_index),
            "SMEMBERS" => smembers(parsed_command, database_holder, db_index),
            "SISMEMBER" => sismember(parsed_command, database_holder, db_index),
            "SCARD" => scard(parsed_command, database_holder, db_index),
            // Hash commands
            "HSET" => hset(parsed_command, database_holder, db_index),
            "HGET" => hget(parsed_command, database_holder, db_index),
            "HGETALL" => hgetall(parsed_command, database_holder, db_index),
            "HDEL" => hdel(parsed_command, database_holder, db_index),
            "HEXISTS" => hexists(parsed_command, database_holder, db_index),
            "HLEN" => hlen(parsed_command, database_holder, db_index),
            // Sorted Set commands
            "ZADD" => zadd(parsed_command, database_holder, db_index),
            "ZRANGE" => zrange(parsed_command, database_holder, db_index),
            "ZREM" => zrem(parsed_command, database_holder, db_index),
            "ZCARD" => zcard(parsed_command, database_holder, db_index),
            "ZSCORE" => zscore(parsed_command, database_holder, db_index),
            // Cluster command
            "CLUSTER" => {
                match cluster_holder {
                    Some(ch) => cluster_command(parsed_command, ch, database_holder, db_index).await,
                    None => Err(anyhow!("ERR This instance has cluster support disabled")),
                }
            }

            _ => {
                info!("Unknown command: {}", command_name);
                Err(anyhow!("ERR unknown command '{}'", command_name))
            }
        };

        match result {
            Ok(r) => r,
            Err(r) => {
                error!("The error is {}", r);
                Response::Error(r.to_string())
            }
        }
    }

    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        let mut buf = vec![0u8; 8192];

        // Read data once
        let n = match self.connect.read(&mut buf).await {
            Ok(0) => {
                info!("Connection closed by client");
                return Err(anyhow!(""));
            }
            Ok(n) => n,
            Err(err) => {
                error!("Error reading data from socket: {}", err);
                return Err(anyhow!(""));
            }
        };

        // Resize buffer to actual data read
        buf.truncate(n);

        // Parse all commands (pipeline support)
        let commands = Request::parse_all(&buf)?;

        let db_index = 0;
        let mut responses = Vec::new();

        // Process each command and collect responses
        for parsed_command in commands {
            // Extract command name for cluster routing check and AOF logging
            let command_name = parsed_command
                .get_str(0)
                .unwrap_or("")
                .to_uppercase();

            // Save RESP bytes for AOF before the command is consumed
            let resp_bytes = if is_write_command(&command_name) {
                Some(parsed_command.get_data())
            } else {
                None
            };

            // Cluster routing check for key-based commands
            if let Some(ref mut ch) = self.cluster_holder {
                if let Some(key_idx) = key_index_for_command(&command_name) {
                    if let Ok(key) = parsed_command.get_slice(key_idx) {
                        match check_cluster_redirect(&command_name, key, ch).await {
                            Ok(()) => {} // proceed
                            Err(redirect_response) => {
                                responses.push(redirect_response.as_bytes());
                                continue;
                            }
                        }
                    }
                }
            }

            let response = Self::process_command(
                parsed_command,
                db_index,
                &mut self.database_holder,
                &mut self.cluster_holder,
            )
            .await;

            // Log to AOF if this was a successful write command
            let is_error = matches!(&response, Response::Error(_));
            if !is_error {
                if let Some(bytes) = resp_bytes {
                    if let Err(e) = self.database_holder.log_to_aof(&bytes) {
                        error!("Failed to log to AOF: {}", e);
                    }
                }
            }

            responses.push(response.as_bytes());
        }

        // Send all responses
        for response_bytes in responses {
            self.connect.write_all(&response_bytes).await?;
        }

        Ok(())
    }
}
