use anyhow::ensure;

use crate::vojo::parsered_command::ParsedCommand;

use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::redis_data::RedisData;

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
pub fn append(
    parser: ParsedCommand,
    redis_data: &mut RedisData,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let value = parser.get_vec(2)?;
    if let Some(existing_value) = redis_data.string_value.get_mut(&key) {
        ensure!(
            existing_value.len() + value.len() < 512 * 1024 * 1024,
            "ERR string exceeds maximum allowed size (512MB)"
        );
        existing_value.extend_from_slice(&value);
    } else {
        redis_data.string_value.insert(key, value);
    }

    Ok(Response::Status("OK".to_owned()))
}
pub fn decr(parser: ParsedCommand, redis_data: &mut RedisData) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    generic_incr(parser, redis_data, -1)
}
pub fn decrby(
    parser: ParsedCommand,
    redis_data: &mut RedisData,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let decrement = parser.get_i64(2)?;
    generic_incr(parser, redis_data, -decrement)
}
pub fn get(parser: ParsedCommand, redis_data: &RedisData) -> Result<Response, anyhow::Error> {
    let key = parser.get_vec(1)?;
    Ok(redis_data
        .string_value
        .get(&key)
        .map(|v| Response::Data(v.clone()))
        .unwrap_or(Response::Nil))
}
pub fn getdel(
    parser: ParsedCommand,
    redis_data: &mut RedisData,
) -> Result<Response, anyhow::Error> {
    let key = parser.get_vec(1)?;
    if let Some(value) = redis_data.string_value.remove(&key) {
        Ok(Response::Data(value))
    } else {
        Ok(Response::Nil)
    }
}
pub fn getex(parser: ParsedCommand, redis_data: &mut RedisData) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 2 && parser.argv.len() <= 4,
        "InvalidArgument"
    );
    let key = parser.get_vec(1)?;
    let response = redis_data
        .string_value
        .get(&key)
        .map(|v| Response::Data(v.clone()))
        .unwrap_or(Response::Nil);
    if parser.argv.len() == 2 {
        return Ok(response);
    };
    let time_unit = parser.get_str(2)?;
    if time_unit == "PERSIST" {
        redis_data.expire_map.remove(&key);
        return Ok(response);
    } else {
        let time_count = parser.get_i64(3)?;

        redis_data
            .expire_map
            .insert(key, time_count * 1000 + mstime());
        return Ok(response);
    }
}
fn generic_incr(
    parser: ParsedCommand,
    redis_data: &mut RedisData,
    increment: i64,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let value = redis_data
        .string_value
        .entry(key.clone())
        .or_insert("0".as_bytes().to_vec());
    let value_integer = std::str::from_utf8(value)?.parse::<i64>()? + increment;
    redis_data
        .string_value
        .insert(key, value_integer.to_string().as_bytes().to_vec());

    Ok(Response::Status("OK".to_owned()))
}
