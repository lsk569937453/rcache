use crate::parser::response::Response;

use crate::vojo::parsered_command::ParsedCommand;

use crate::database::lib::DatabaseHolder;
use anyhow::{anyhow, ensure};
pub  fn zadd(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 2, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
    let key = parser.get_vec(1)?;
    let mut i = 2;
    let mut count = 0;
    loop {
        let score = parser.get_f64(i)?;
        let member = parser.get_vec(i + 1)?;
        i += 2;
        if db.zadd(db_index, key.clone(), score, member)? {
            count += 1;
        }
        if i >= parser.argv.len() {
            break;
        }
    }
    Ok(Response::Integer(count as i64))
}
pub  fn zrange(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 4, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let start = parser.get_i64(2)?;
    let stop = parser.get_i64(3)?;
    let members = db.zrange(db_index, key, start, stop)?;
    Ok(Response::Array(
        members.into_iter().map(Response::Data).collect(),
    ))
}
pub  fn zrem(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let mut count = 0;
    for i in 2..parser.argv.len() {
        let member = parser.get_vec(i)?;
        if db.zrem(db_index, key.clone(), member)? {
            count += 1;
        }
    }
    Ok(Response::Integer(count))
}
pub  fn zcard(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let count = db.zcard(db_index, key)?;
    Ok(Response::Integer(count as i64))
}
pub  fn zscore(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let member = parser.get_vec(2)?;
    let result = db.zscore(db_index, key, member)?;
    match result {
        Some(score) => {
            let score_str = format!("{}", score);
            Ok(Response::Data(score_str.as_bytes().to_vec()))
        }
        None => Ok(Response::Nil),
    }
}
