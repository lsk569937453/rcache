use anyhow::{anyhow, ensure};
use chrono::Utc;

use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;

use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueString;
use std::time::Duration;
pub fn set(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    if let Some(value) = database.get(db_index, key.clone())? {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value = parser.get_vec(2)?;
    let _nx = false;
    let _xx = false;
    let _skip = false;

    let value = ValueString { data: value };
    let wrapped_value = Value::String(value);
    database.insert(db_index, key, wrapped_value)?;
    Ok(Response::Status("OK".to_owned()))
}

pub fn get(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let val_option = database.get(dbindex, key)?;
    if let Some(value) = val_option {
        ensure!(value.is_string(), "InvalidArgument");
        Ok(Response::Data(value.to_value_string()?.data))
    } else {
        Ok(Response::Nil)
    }
}
pub fn incr(
    parser: ParsedCommand,
    db: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");

    generic_incr(parser, db, dbindex, 1)
}
fn generic_incr(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    dbindex: usize,
    increment: i64,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let mut db = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let option_val = db.get(dbindex, key.clone())?;
    if let Some(value) = option_val {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value = match option_val {
        Some(v) => {
            let data = v.to_value_string()?.data;
            std::str::from_utf8(&data)?.parse::<i64>()?
        }
        None => 0,
    };
    let value_integer = value + increment;

    db.insert(
        dbindex,
        key,
        Value::String(ValueString {
            data: value_integer.to_string().as_bytes().to_vec(),
        }),
    )?;

    Ok(Response::Status("OK".to_owned()))
}
pub fn del(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 2,
        "InvalidArgument: wrong number of arguments for 'del' command"
    );

    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let mut deleted_count = 0;

    for i in 1..parser.argv.len() {
        let key = parser.get_vec(i)?;
        if database.remove(db_index, &key)?.is_some() {
            deleted_count += 1;
        }
    }

    Ok(Response::Integer(deleted_count))
}
pub fn exists(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 2,
        "InvalidArgument: wrong number of arguments for 'exists' command"
    );

    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let mut exists_count = 0;

    for i in 1..parser.argv.len() {
        let key = parser.get_vec(i)?;
        if database.contains_key(db_index, &key)? {
            exists_count += 1;
        }
    }

    Ok(Response::Integer(exists_count))
}
pub fn expire(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() == 3,
        "InvalidArgument: wrong number of arguments for 'expire' command"
    );

    let key = parser.get_vec(1)?;
    let seconds_str = parser.get_str(2)?;
    let seconds = seconds_str
        .parse::<u64>()
        .map_err(|_| anyhow!("ERR value is not an integer or out of range"))?;

    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;

    if !database.contains_key(db_index, &key)? {
        return Ok(Response::Integer(0));
    }

    let expire_at = Utc::now() + Duration::from_secs(seconds);
    database.set_expire(db_index, key, expire_at.timestamp() as u64)?;

    Ok(Response::Integer(1))
}

pub fn ttl(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() == 2,
        "InvalidArgument: wrong number of arguments for 'ttl' command"
    );

    let key = parser.get_vec(1)?;
    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;

    let res = database.get_ttl(db_index, &key)?;
    Ok(Response::Integer(res))
}
