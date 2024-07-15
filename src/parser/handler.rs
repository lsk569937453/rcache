use crate::command::hash_command::hset;
use crate::command::list_command::{lpop, lpush, lrange, rpop, rpush};
use crate::command::set_command::sadd;
use crate::command::sorted_set_command::zadd;
use crate::command::string_command::{get, incr, set};
use crate::database::lib::DatabaseHolder;
use crate::parser::ping::ping;
use crate::parser::request::Request;
use crate::parser::response::Response;
use monoio::io::AsyncReadRent;
use monoio::io::AsyncWriteRentExt;
use monoio::net::TcpStream;
pub struct Handler {
    pub connect: TcpStream,
    pub database_holder: DatabaseHolder,
}

impl Handler {
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        let mut buf = vec![0u8; 1024];
        let (res, parsed_command) = self.connect.read(buf).await;
        if res? == 0 {
            info!("Connection closed by client");
            return Err(anyhow!(""));
        };

        let (parsed_command, _) = Request::parse_buf(&parsed_command)?;
        let db_index = 0;
        let database_holder = &mut self.database_holder;
        let command_name = parsed_command.get_str(0)?.to_uppercase();
        let result = match command_name.as_str() {
            "PING" => ping(parsed_command),
            "SET" => set(parsed_command, database_holder, db_index),
            "GET" => get(parsed_command, database_holder, db_index),
            "LPUSH" => lpush(parsed_command, database_holder, db_index),
            "RPUSH" => rpush(parsed_command, database_holder, db_index),
            "LPOP" => lpop(parsed_command, database_holder, db_index),
            "RPOP" => rpop(parsed_command, database_holder, db_index),
            "SADD" => sadd(parsed_command, database_holder, db_index),
            "HSET" => hset(parsed_command, database_holder, db_index),
            "ZADD" => zadd(parsed_command, database_holder, db_index),
            "LRANGE" => lrange(parsed_command, database_holder, db_index),
            "INCR" => incr(parsed_command, database_holder, db_index),

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
        self.connect.write_all(data.as_bytes()).await;
        Ok(())
    }
}
