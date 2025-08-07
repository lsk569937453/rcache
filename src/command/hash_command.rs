use anyhow::{anyhow, ensure};

use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;

pub fn hset(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 3, "InvalidArgument");
    let mut db = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
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
pub fn hget(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() == 3,
        "InvalidArgument: wrong number of arguments for 'hget' command"
    );

    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let field = parser.get_vec(2)?;

    match database.get(db_index, key)? {
        Some(value) => {
            ensure!(
                value.is_hash(),
                "WRONGTYPE Operation against a key holding the wrong kind of value"
            );
            let hash_value = value.to_value_hash()?;
            match hash_value.data.get(&field) {
                Some(field_value) => Ok(Response::Data(field_value.clone())),
                None => Ok(Response::Nil),
            }
        }
        None => Ok(Response::Nil),
    }
}

pub fn hgetall(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() == 2,
        "InvalidArgument: wrong number of arguments for 'hgetall' command"
    );

    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;

    match database.get(db_index, key)? {
        Some(value) => {
            ensure!(
                value.is_hash(),
                "WRONGTYPE Operation against a key holding the wrong kind of value"
            );
            let hash_value = value.to_value_hash()?;

            let mut result_array = Vec::with_capacity(hash_value.data.len() * 2);
            for (field, val) in hash_value.data.iter() {
                result_array.push(Response::Data(field.clone()));
                result_array.push(Response::Data(val.clone()));
            }
            Ok(Response::Array(result_array))
        }
        None => Ok(Response::Array(Vec::new())),
    }
}
