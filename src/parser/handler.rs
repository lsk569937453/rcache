use crate::command::hash_command::{hdel, hget, hgetall, hexists, hlen, hset};
use crate::command::list_command::{llen, lpop, lpush, lrange, rpop, rpush};
use crate::command::set_command::{sadd, scard, sismember, smembers, srem};
use crate::command::sorted_set_command::{zadd, zcard, zrange, zrem, zscore};
use crate::command::string_command::{
    append, decr, decrby, del, exists, get, getdel, getrange, getset, incr, incrby,
    incrbyfloat, mget, mset, msetnx, set, strlen, type_cmd,
};
use crate::database::lib::DatabaseHolder;
use crate::parser::ping::ping;
use crate::parser::request::Request;
use crate::parser::response::Response;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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

            _ => {
                info!("{}", command_name);
                Ok(Response::Nil)
            }
        };
        let data = match result {
            Ok(r) => r,
            Err(r) => {
                error!("The error is {}", r);
                Response::Error(r.to_string())
            }
        };
        self.connect.write_all(&data.as_bytes()).await?;
        Ok(())
    }
}
