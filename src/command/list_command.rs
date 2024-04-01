use std::collections::LinkedList;

use anyhow::{anyhow, ensure};

use crate::database::lib::Database;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueList;
use crate::vojo::value::ValueString;
pub fn lpush(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 2, "InvalidArgument");
    let key = parser.get_vec(1)?;

    let mut len = 0;
    for i in 2..parser.argv.len() {
        let val = parser.get_vec(i)?;
        len = db.lpush(db_index, key.clone(), val)?;
    }

    Ok(Response::Integer(len as i64))
}
pub fn rpush(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 2, "InvalidArgument");
    let key = parser.get_vec(1)?;

    let mut len = 0;
    for i in 2..parser.argv.len() {
        let val = parser.get_vec(i)?;
        len = db.rpush(db_index, key.clone(), val)?;
    }

    Ok(Response::Integer(len as i64))
}
pub fn lpop(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 2, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let count_option = if parser.argv.len() == 3 {
        Some(parser.get_i64(2)?)
    } else {
        None
    };
    let res = db.lpop(db_index, key, count_option)?;
    Ok(res)
}
pub fn rpop(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 2, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let count_option = if parser.argv.len() == 3 {
        Some(parser.get_i64(2)?)
    } else {
        None
    };
    let res = db.rpop(db_index, key, count_option)?;
    Ok(res)
}
pub fn lrange(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 4, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let start = parser.get_i64(2)?;
    let stop = parser.get_i64(3)?;

    if start > stop {
        return Err(anyhow!("InvalidArgument"));
    }
    let res = db.lrange(db_index, key, start, stop)?;
    Ok(res)
}
