use anyhow::{anyhow, ensure};

use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;

pub  fn hset(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
    let key = parser.get_vec(1)?;
    let mut len = 0;
    for i in 0..(parser.argv.len() - 2) / 2 {
        let field = parser.get_vec(2 * i + 2)?;
        let val = parser.get_vec(2 * i + 3)?;
        if db.hset(db_index, key.clone(), field, val)? {
            len += 1;
        }
    }

    Ok(Response::Integer(len as i64))
}
pub  fn hget(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let field = parser.get_vec(2)?;
    let result = db.hget(db_index, key, field)?;
    match result {
        Some(v) => Ok(Response::Data(v)),
        None => Ok(Response::Nil),
    }
}
pub  fn hgetall(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let pairs = db.hgetall(db_index, key)?;
    let mut responses = vec![];
    for (field, value) in pairs {
        responses.push(Response::Data(field));
        responses.push(Response::Data(value));
    }
    Ok(Response::Array(responses))
}
pub  fn hdel(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let mut count = 0;
    for i in 2..parser.argv.len() {
        let field = parser.get_vec(i)?;
        if db.hdel(db_index, key.clone(), field)? {
            count += 1;
        }
    }
    Ok(Response::Integer(count))
}
pub  fn hexists(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let field = parser.get_vec(2)?;
    let result = db.hexists(db_index, key, field)?;
    if result {
        Ok(Response::Integer(1))
    } else {
        Ok(Response::Integer(0))
    }
}
pub  fn hlen(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let len = db.hlen(db_index, key)?;
    Ok(Response::Integer(len as i64))
}
