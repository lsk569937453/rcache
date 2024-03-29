use anyhow::ensure;

use crate::vojo::parsered_command::ParsedCommand;

use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::redis_data::RedisData;
use std::time::{SystemTime, UNIX_EPOCH};
pub fn get(parser: ParsedCommand, redis_data: &RedisData) -> Result<Response, anyhow::Error> {
    let key = parser.get_vec(1)?;
    Ok(redis_data
        .string_value
        .get(&key)
        .map(|v| Response::Data(v.clone()))
        .unwrap_or(Response::Nil))
}
pub fn set(parser: ParsedCommand, redis_data: &mut RedisData) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let value = parser.get_vec(2)?;
    redis_data.string_value.insert(key, value);
    Ok(Response::Status("OK".to_owned()))
}
pub fn setex(parser: ParsedCommand, redis_data: &mut RedisData) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 4, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let seconds = parser.get_i64(2)?;
    let value = parser.get_vec(3)?;
    redis_data.string_value.insert(key.clone(), value);
    redis_data
        .expire_map
        .insert(key.clone(), seconds * 1000 + mstime());
    Ok(Response::Status("OK".to_owned()))
}
