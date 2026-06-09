use anyhow::{anyhow, ensure};

use crate::parser::response::Response;

use crate::vojo::parsered_command::ParsedCommand;

use crate::database::lib::DatabaseHolder;
use crate::database::lru::estimate_value_memory;

pub  fn sadd(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
    let key = parser.get_vec(1)?;
    let old_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    let mut count = 0;
    for i in 2..parser.argv.len() {
        let val = parser.get_vec(i)?;
        if db.sadd(db_index, key.clone(), val)? {
            count += 1;
        }
    }
    let new_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    drop(db);
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(db_index, &key);
    database_lock.evict_if_needed()?;
    Ok(Response::Integer(count))
}
pub  fn srem(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let old_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    let mut count = 0;
    for i in 2..parser.argv.len() {
        let val = parser.get_vec(i)?;
        if db.srem(db_index, key.clone(), val)? {
            count += 1;
        }
    }
    let new_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    drop(db);
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Integer(count))
}
pub  fn smembers(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let members = db.smembers(db_index, key.clone())?;
    drop(db);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Array(
        members.into_iter().map(Response::Data).collect(),
    ))
}
pub  fn sismember(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let member = parser.get_vec(2)?;
    let result = db.sismember(db_index, key.clone(), member)?;
    drop(db);
    database_lock.lru_touch(db_index, &key);
    if result {
        Ok(Response::Integer(1))
    } else {
        Ok(Response::Integer(0))
    }
}
pub  fn scard(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let count = db.scard(db_index, key.clone())?;
    drop(db);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Integer(count as i64))
}
