use anyhow::{anyhow, ensure};
use chrono::Utc;

use crate::database::lib::DatabaseHolder;
use crate::database::lru::estimate_value_memory;
use crate::parser::response::Response;

use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueString;
pub  fn set(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut database = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
    let key = parser.get_vec(1)?;
    // Estimate old value memory before overwrite
    let old_mem = database.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    if let Some(value) = database.get(db_index, key.clone())? {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value = parser.get_vec(2)?;
    let value = ValueString { data: value };
    let wrapped_value = Value::String(value);
    let new_mem = estimate_value_memory(&wrapped_value);
    database.insert(db_index, key.clone(), wrapped_value)?;
    drop(database);
    // Update LRU and memory tracking
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(db_index, &key);
    database_lock.evict_if_needed()?;
    Ok(Response::Status("OK".to_owned()))
}

pub  fn get(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let database = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
    let key = parser.get_vec(1)?;
    let val_option = database.get(dbindex, key.clone())?;
    let result = if let Some(value) = val_option {
        ensure!(value.is_string(), "InvalidArgument");
        Ok(Response::Data(value.to_value_string()?.data))
    } else {
        Ok(Response::Nil)
    };
    drop(database);
    // Read also counts as LRU access
    database_lock.lru_touch(dbindex, &key);
    result
}
pub  fn incr(
    parser: ParsedCommand,
    db: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    generic_incr(parser, db, dbindex, 1)
}
pub  fn decr(
    parser: ParsedCommand,
    db: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    generic_incr(parser, db, dbindex, -1)
}
pub  fn incrby(
    parser: ParsedCommand,
    db: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let increment = parser.get_i64(2)?;
    generic_incr(parser, db, dbindex, increment)
}
pub  fn decrby(
    parser: ParsedCommand,
    db: &mut DatabaseHolder,
    dbindex: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let decrement = parser.get_i64(2)?;
    generic_incr(parser, db, dbindex, -decrement)
}
 fn generic_incr(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    dbindex: usize,
    increment: i64,
) -> Result<Response, anyhow::Error> {
    let mut db = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
    let key = parser.get_vec(1)?;
    let old_mem = db.get(dbindex, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    let option_val = db.get(dbindex, key.clone())?;
    if let Some(value) = &option_val {
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
    let new_value = Value::String(ValueString {
        data: value_integer.to_string().as_bytes().to_vec(),
    });
    let new_mem = estimate_value_memory(&new_value);
    db.insert(
        dbindex,
        key.clone(),
        new_value,
    )?;
    drop(db);
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(dbindex, &key);
    database_lock.evict_if_needed()?;

    Ok(Response::Status("OK".to_owned()))
}
pub  fn incrbyfloat(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let old_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    let increment = parser.get_f64(2)?;
    let option_val = db.get(db_index, key.clone())?;
    if let Some(value) = &option_val {
        ensure!(value.is_string(), "InvalidArgument");
    }
    let value: f64 = match option_val {
        Some(v) => {
            let data = v.to_value_string()?.data;
            std::str::from_utf8(&data)?.parse::<f64>()?
        }
        None => 0.0,
    };
    let value_float = value + increment;
    let value_str = format!("{}", value_float);
    let new_value = Value::String(ValueString {
        data: value_str.as_bytes().to_vec(),
    });
    let new_mem = estimate_value_memory(&new_value);
    db.insert(db_index, key.clone(), new_value)?;
    drop(db);
    let result = Response::Data(value_str.as_bytes().to_vec());
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(db_index, &key);
    database_lock.evict_if_needed()?;
    Ok(result)
}
pub  fn append(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let value = parser.get_vec(2)?;
    let old_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    let len = db.append(db_index, key.clone(), value)?;
    let new_mem = db.get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    drop(db);
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(db_index, &key);
    database_lock.evict_if_needed()?;
    Ok(Response::Integer(len as i64))
}
pub  fn strlen(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let len = db.strlen(db_index, key.clone())?;
    drop(db);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Integer(len as i64))
}
pub  fn getrange(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 4, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let mut start = parser.get_i64(2)?;
    let mut end = parser.get_i64(3)?;
    let val_option = db.get(db_index, key.clone())?;
    let result = match val_option {
        Some(v) => {
            ensure!(v.is_string(), "InvalidArgument");
            let data = &v.to_value_string()?.data;
            let len = data.len() as i64;
            if start < 0 {
                start = len + start;
            }
            if start < 0 {
                start = 0;
            }
            if end < 0 {
                end = len + end;
            }
            if end < 0 || start > end || start >= len {
                return Ok(Response::Data(vec![]));
            }
            let end = (end.min(len - 1)) as usize + 1;
            let start = start as usize;
            Ok(Response::Data(data[start..end].to_vec()))
        }
        None => Ok(Response::Data(vec![])),
    };
    drop(db);
    database_lock.lru_touch(db_index, &key);
    result
}
pub  fn getset(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let old_mem = db
        .get(db_index, key.clone())?
        .map(|v| estimate_value_memory(v))
        .unwrap_or(0);
    let old_value = db
        .get(db_index, key.clone())?
        .map(|v| v.to_value_string())
        .transpose()?
        .map(|v| v.data);
    let value = parser.get_vec(2)?;
    let wrapped_value = Value::String(ValueString { data: value });
    let new_mem = estimate_value_memory(&wrapped_value);
    db.insert(db_index, key.clone(), wrapped_value)?;
    drop(db);
    database_lock.memory_sub(old_mem);
    database_lock.memory_add(new_mem);
    database_lock.lru_touch(db_index, &key);
    database_lock.evict_if_needed()?;
    match old_value {
        Some(data) => Ok(Response::Data(data)),
        None => Ok(Response::Nil),
    }
}
pub  fn getdel(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let val_option = db.get(db_index, key.clone())?;
    let result = match val_option {
        Some(v) => {
            ensure!(v.is_string(), "InvalidArgument");
            let mem = estimate_value_memory(v);
            let data = v.to_value_string()?.data;
            db.remove(db_index, key.clone())?;
            drop(db);
            database_lock.memory_sub(mem);
            database_lock.lru_remove(db_index, &key);
            Ok(Response::Data(data))
        }
        None => {
            drop(db);
            Ok(Response::Nil)
        }
    };
    result
}
pub  fn mget(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let mut responses = vec![];
    let mut keys = vec![];
    for i in 1..parser.argv.len() {
        let key = parser.get_vec(i)?;
        keys.push(key.clone());
        let val_option = db.get(db_index, key)?;
        match val_option {
            Some(value) => {
                if value.is_string() {
                    responses.push(Response::Data(value.to_value_string()?.data));
                } else {
                    responses.push(Response::Nil);
                }
            }
            None => responses.push(Response::Nil),
        }
    }
    drop(db);
    for key in &keys {
        database_lock.lru_touch(db_index, key);
    }
    Ok(Response::Array(responses))
}
pub  fn mset(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 3 && (parser.argv.len() - 1) % 2 == 0,
        "InvalidArgument"
    );
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let mut mem_delta: i64 = 0;
    let mut keys = vec![];
    for i in 0..(parser.argv.len() - 1) / 2 {
        let key = parser.get_vec(2 * i + 1)?;
        keys.push(key.clone());
        let old_mem = db.get(db_index, key.clone())?
            .map(|v| estimate_value_memory(v))
            .unwrap_or(0);
        let value = parser.get_vec(2 * i + 2)?;
        let wrapped_value = Value::String(ValueString { data: value });
        let new_mem = estimate_value_memory(&wrapped_value);
        mem_delta += new_mem as i64 - old_mem as i64;
        db.insert(db_index, key, wrapped_value)?;
    }
    drop(db);
    if mem_delta > 0 {
        database_lock.memory_add(mem_delta as usize);
    } else {
        database_lock.memory_sub((-mem_delta) as usize);
    }
    for key in &keys {
        database_lock.lru_touch(db_index, key);
    }
    database_lock.evict_if_needed()?;
    Ok(Response::Status("OK".to_owned()))
}
pub  fn msetnx(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() >= 3 && (parser.argv.len() - 1) % 2 == 0,
        "InvalidArgument"
    );
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    // First check if any key already exists
    for i in 0..(parser.argv.len() - 1) / 2 {
        let key = parser.get_vec(2 * i + 1)?;
        if db.get(db_index, key)?.is_some() {
            return Ok(Response::Integer(0));
        }
    }
    // None exist, set all
    let mut mem_delta: i64 = 0;
    let mut keys = vec![];
    for i in 0..(parser.argv.len() - 1) / 2 {
        let key = parser.get_vec(2 * i + 1)?;
        keys.push(key.clone());
        let value = parser.get_vec(2 * i + 2)?;
        let wrapped_value = Value::String(ValueString { data: value });
        let new_mem = estimate_value_memory(&wrapped_value);
        mem_delta += new_mem as i64;
        db.insert(db_index, key, wrapped_value)?;
    }
    drop(db);
    if mem_delta > 0 {
        database_lock.memory_add(mem_delta as usize);
    }
    for key in &keys {
        database_lock.lru_touch(db_index, key);
    }
    database_lock.evict_if_needed()?;
    Ok(Response::Integer(1))
}
pub  fn del(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 2, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let mut count = 0;
    let mut keys_to_remove = vec![];
    for i in 1..parser.argv.len() {
        let key = parser.get_vec(i)?;
        let mem = db.get(db_index, key.clone())?
            .map(|v| estimate_value_memory(v))
            .unwrap_or(0);
        if db.remove(db_index, key.clone())? {
            database_lock.memory_sub(mem);
            keys_to_remove.push(key);
            count += 1;
        }
    }
    drop(db);
    for key in &keys_to_remove {
        database_lock.lru_remove(db_index, key);
    }
    Ok(Response::Integer(count))
}
pub  fn exists(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let mut count = 0;
    let mut keys = vec![];
    for i in 1..parser.argv.len() {
        let key = parser.get_vec(i)?;
        keys.push(key.clone());
        if db.get(db_index, key)?.is_some() {
            count += 1;
        }
    }
    drop(db);
    for key in &keys {
        database_lock.lru_touch(db_index, key);
    }
    Ok(Response::Integer(count))
}
pub  fn type_cmd(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let val_option = db.get(db_index, key.clone())?;
    let type_str = match val_option {
        Some(v) => match v {
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Set(_) => "set",
            Value::Hash(_) => "hash",
            Value::SortedSet(_) => "zset",
            Value::Nil => "none",
        },
        None => "none",
    };
    drop(db);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Status(type_str.to_owned()))
}

/// DBSIZE - Return the number of keys in the current database
pub fn dbsize(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 1, "InvalidArgument");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key_count = db.data[db_index].len();
    Ok(Response::Integer(key_count as i64))
}

/// KEYS - Return all keys matching the pattern
/// Currently only supports "*" pattern (all keys)
pub fn keys(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "InvalidArgument");
    let pattern = parser.get_str(1)?;

    // Currently only support "*" pattern
    if pattern != "*" {
        return Ok(Response::Array(vec![])); // Empty array for unsupported patterns
    }

    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let keys: Vec<Response> = db.data[db_index]
        .keys()
        .map(|key| Response::Data(key.clone()))
        .collect();

    Ok(Response::Array(keys))
}

/// EXPIRE key seconds — Set a timeout on a key
/// Returns 1 if the timeout was set, 0 if the key does not exist
pub fn expire(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 3, "wrong number of arguments for 'expire' command");
    let mut db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let seconds = parser.get_i64(2)?;
    let expire_at = Utc::now().timestamp() + seconds;
    let result = db.set_expire(db_index, key.clone(), expire_at)?;
    drop(db);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Integer(if result { 1 } else { 0 }))
}

/// TTL key — Get the remaining time to live of a key in seconds
/// Returns -2 if the key does not exist, -1 if no expiration, or remaining seconds
pub fn ttl(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() == 2, "wrong number of arguments for 'ttl' command");
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let remaining = db.get_ttl(db_index, key.clone())?;
    drop(db);
    database_lock.lru_touch(db_index, &key);
    Ok(Response::Integer(remaining))
}
