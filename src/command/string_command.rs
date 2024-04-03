use anyhow::{anyhow, ensure};

use crate::database::lib::Database;
use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueString;
pub fn set(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!(""))?;
    let key = parser.get_vec(1)?;
    if let Some(value) = database.get(db_index, key.clone())? {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value = parser.get_vec(2)?;
    let mut nx = false;
    let mut xx = false;
    let mut skip = false;

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
    let database = database_lock
        .database_lock
        .lock()
        .map_err(|_| anyhow!(""))?;
    let key = parser.get_vec(1)?;
    let val_option = database.get(dbindex, key)?;
    if let Some(value) = val_option {
        ensure!(value.is_string(), "InvalidArgument");
        Ok(Response::Data(value.to_value_string()?.data))
    } else {
        Ok(Response::Nil)
    }
}
