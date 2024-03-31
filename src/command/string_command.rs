use anyhow::{anyhow, ensure};

use crate::database::lib::Database;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueString;
pub fn set(
    parser: ParsedCommand,
    database: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");

    let key = parser.get_vec(1)?;
    if let Some(value) = database.get(db_index, key.clone())? {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value = parser.get_vec(2)?;
    let mut nx = false;
    let mut xx = false;
    let mut expiration = None;
    let mut skip = false;
    for i in 3..parser.argv.len() {
        if skip {
            skip = false;
            continue;
        }
        let param = parser.get_str(i)?;
        match &*param.to_ascii_lowercase() {
            "nx" => nx = true,
            "xx" => xx = true,
            "px" => {
                let px = parser.get_i64(i + 1)?;
                expiration = Some(px);
                skip = true;
            }
            "ex" => {
                let ex = parser.get_i64(i + 1)?;
                expiration = Some(ex * 1000);
                skip = true;
            }
            _ => return Ok(Response::Error("ERR syntax error".to_owned())),
        }
    }
    let value = ValueString { data: value };
    let wrapped_value = Value::String(value);
    database.insert(db_index, key, wrapped_value)?;
    Ok(Response::Status("OK".to_owned()))
}

pub fn setex(
    parser: ParsedCommand,
    database: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 4, "InvalidArgument");
    let key = parser.get_vec(1)?;
    if let Some(value) = database.get(db_index, key.clone())? {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let seconds = parser.get_i64(2)?;
    let value = parser.get_vec(3)?;

    database.insert(
        db_index,
        key.clone(),
        Value::String(ValueString { data: value }),
    )?;
    database.expire_insert(db_index, key, seconds * 1000 + mstime())?;

    Ok(Response::Status("OK".to_owned()))
}
pub fn append(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let value_option = db.get_mut(db_index, key.clone())?;
    let value = parser.get_vec(2)?;
    if let Some(existing_value) = value_option {
        ensure!(existing_value.is_string(), "InvalidArgument");

        let str_len = existing_value.strlen()?;
        ensure!(
            str_len + value.len() < 512 * 1024 * 1024,
            "ERR string exceeds maximum allowed size (512MB)"
        );
        existing_value.append(value)?;
    } else {
        db.insert(db_index, key, Value::String(ValueString { data: value }))?;
    }

    Ok(Response::Status("OK".to_owned()))
}
pub fn decr(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    generic_incr(parser, db, dbindex, -1)
}
pub fn decrby(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let decrement = parser.get_i64(2)?;
    generic_incr(parser, db, dbindex, -decrement)
}
pub fn get(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");

    let key = parser.get_vec(1)?;
    let val_option = db.get(dbindex, key)?;
    if let Some(value) = val_option {
        ensure!(value.is_string(), "InvalidArgument");
        Ok(Response::Data(value.to_value_string()?.data))
    } else {
        Ok(Response::Nil)
    }
}
pub fn getdel(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let key = parser.get_vec(1)?;
    if let Some(value) = db.get(dbindex, key.clone())? {
        let data = value.to_value_string()?.data;
        db.remove(dbindex, key)?;
        Ok(Response::Data(data))
    } else {
        Ok(Response::Nil)
    }
}
pub fn getex(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 2 && parser.argv.len() <= 4,
        "InvalidArgument"
    );
    let key = parser.get_vec(1)?;
    let response = match db.get(dbindex, key.clone())? {
        Some(value) => Response::Data(value.to_value_string()?.data),
        None => Response::Nil,
    };
    if parser.argv.len() == 2 {
        return Ok(response);
    };
    let time_unit = parser.get_str(2)?;
    if time_unit == "PERSIST" {
        db.expire_remove(dbindex, key)?;
        Ok(response)
    } else {
        let time_count = parser.get_i64(3)?;
        db.expire_insert(dbindex, key, time_count * 1000 + mstime())?;
        Ok(response)
    }
}
fn generic_incr(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
    increment: i64,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
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
pub fn getrange(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    Err(anyhow!("todo!"))
}
pub fn getset(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let value = parser.get_vec(2)?;

    let response = match db.get(dbindex, key.clone())? {
        Some(r) => Response::Data(r.to_value_string()?.data),
        None => Response::Nil,
    };
    db.insert(dbindex, key, Value::String(ValueString { data: value }))?;
    Ok(response)
}
pub fn incr(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    generic_incr(parser, db, dbindex, 1)
}
pub fn incrby(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let decrement = parser.get_i64(2)?;
    generic_incr(parser, db, dbindex, decrement)
}
pub fn incrbyfloat(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let key = parser.get_vec(1)?;
    let increment = parser.get_f64(2)?;
    let option_val = db.get(dbindex, key.clone())?;
    if let Some(value) = option_val {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value = match option_val {
        Some(v) => {
            let data = v.to_value_string()?.data;
            std::str::from_utf8(&data)?.parse::<f64>()?
        }
        None => 0.0,
    };
    let value_integer = value + increment;
    let value_vec = value_integer.to_string().as_bytes().to_vec();
    db.insert(
        dbindex,
        key,
        Value::String(ValueString {
            data: value_vec.clone(),
        }),
    )?;

    Ok(Response::Data(value_vec))
}
pub fn lcs(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    Err(anyhow!("todo!"))
}
pub fn mget(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 2, "InvalidArgument");

    let mut responses = Vec::new();
    for i in 1..parser.argv.len() {
        let key = parser.get_vec(i)?;
        let value = match db.get(dbindex, key)? {
            Some(v) => Response::Data(v.to_value_string()?.data),
            None => Response::Nil,
        };
        responses.push(value);
    }
    Ok(Response::Array(responses))
}
pub fn mset(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 3 && parser.argv.len() % 2 == 1,
        "InvalidArgument"
    );

    for i in 0..(parser.argv.len() as i32) / 2 {
        let pos = (i * 2 + 1) as usize;
        let key = parser.get_vec(pos)?;
        let value = parser.get_vec(pos + 1)?;

        db.insert(dbindex, key, Value::String(ValueString { data: value }))?;
    }
    Ok(Response::Status("OK".to_owned()))
}
pub fn msetnx(
    parser: ParsedCommand,
    db: &mut Database,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    Err(anyhow!("todo!"))
}
