use crate::command::parser::ParsedCommand;

use crate::command::response::Response;

use super::redis_data::RedisData;
pub fn get(parser: ParsedCommand, redis_data: &RedisData) -> Result<Response, anyhow::Error> {
    let key = parser.get_vec(1)?;
    Ok(redis_data
        .map
        .get(&key)
        .map(|v| Response::Data(v.clone()))
        .unwrap_or(Response::Nil))
}
pub fn set(parser: ParsedCommand, redis_data: &mut RedisData) -> Result<Response, anyhow::Error> {
    let key = parser.get_vec(1)?;
    let value = parser.get_vec(2)?;
    redis_data.map.insert(key, value);
    Ok(Response::Status("OK".to_owned()))
}
