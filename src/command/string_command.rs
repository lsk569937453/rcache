use anyhow::{anyhow, ensure};

use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;

use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueString;
pub async fn set(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut database = database_lock.database_lock.lock().await;
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

pub async fn get(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let database = database_lock.database_lock.lock().await;
    let key = parser.get_vec(1)?;
    let val_option = database.get(dbindex, key)?;
    if let Some(value) = val_option {
        ensure!(value.is_string(), "InvalidArgument");
        Ok(Response::Data(value.to_value_string()?.data))
    } else {
        Ok(Response::Nil)
    }
}
pub async fn incr(
    parser: ParsedCommand,
    db: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");

    generic_incr(parser, db, dbindex, 1).await
}
async fn generic_incr(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    dbindex: usize,
    increment: i64,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().await;
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
