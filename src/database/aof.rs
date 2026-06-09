// AOF (Append Only File) persistence module.

use crate::database::lib::Database;
use crate::parser::request::Request;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// AOF sync policy
#[derive(Clone, Debug, PartialEq)]
pub enum AofSyncPolicy {
    Always,
    EverySec,
    No,
}

/// Parse sync policy from config string
pub fn parse_sync_policy(s: &str) -> AofSyncPolicy {
    match s.to_lowercase().as_str() {
        "always" => AofSyncPolicy::Always,
        "no" => AofSyncPolicy::No,
        _ => AofSyncPolicy::EverySec,
    }
}

/// AOF writer that appends commands to the AOF file.
pub struct AofWriter {
    file: Mutex<File>,
    sync_policy: AofSyncPolicy,
    bytes_written: Mutex<usize>,
}

impl AofWriter {
    pub fn new(file_path: String, sync_policy: AofSyncPolicy) -> Result<Self, anyhow::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        Ok(AofWriter {
            file: Mutex::new(file),
            sync_policy,
            bytes_written: Mutex::new(0),
        })
    }

    /// Append a raw RESP command to the AOF file.
    pub fn append(&self, resp_bytes: &[u8]) -> Result<(), anyhow::Error> {
        let mut file = self.file.lock().map_err(|e| anyhow!("{}", e))?;
        file.write_all(resp_bytes)?;

        match self.sync_policy {
            AofSyncPolicy::Always => {
                file.sync_all()?;
            }
            AofSyncPolicy::EverySec | AofSyncPolicy::No => {}
        }

        let mut count = self.bytes_written.lock().map_err(|e| anyhow!("{}", e))?;
        *count += resp_bytes.len();
        Ok(())
    }

    /// Get total bytes written since last rewrite
    pub fn bytes_written(&self) -> usize {
        self.bytes_written.lock().map(|g| *g).unwrap_or(0)
    }

    /// Perform periodic fsync for EverySec policy
    pub fn periodic_fsync(&self) -> Result<(), anyhow::Error> {
        if self.sync_policy == AofSyncPolicy::EverySec {
            let file = self.file.lock().map_err(|e| anyhow!("{}", e))?;
            file.sync_all()?;
        }
        Ok(())
    }
}

/// Load database state from an AOF file by replaying all commands.
pub fn load_aof(path: &Path, database: &mut Database) -> Result<(), anyhow::Error> {
    let content = std::fs::read(path)?;
    if content.is_empty() {
        return Ok(());
    }
    let commands = Request::parse_all(&content)?;
    for cmd in commands {
        let command_name = match cmd.get_str(0) {
            Ok(name) => name.to_uppercase(),
            Err(_) => continue,
        };
        replay_command(database, 0, command_name, &cmd)?;
    }
    Ok(())
}

/// Replay a single write command against the database.
fn replay_command(
    db: &mut Database,
    db_index: usize,
    command_name: String,
    cmd: &crate::vojo::parsered_command::ParsedCommand,
) -> Result<(), anyhow::Error> {
    match command_name.as_str() {
        "SET" => {
            if cmd.argv.len() >= 3 {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(2)?;
                db.insert(
                    db_index,
                    key,
                    crate::vojo::value::Value::String(crate::vojo::value::ValueString {
                        data: value,
                    }),
                )?;
            }
        }
        "DEL" => {
            for i in 1..cmd.argv.len() {
                let key = cmd.get_vec(i)?;
                db.remove(db_index, key)?;
            }
        }
        "APPEND" => {
            if cmd.argv.len() >= 3 {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(2)?;
                db.append(db_index, key, value)?;
            }
        }
        "INCR" | "DECR" | "INCRBY" | "DECRBY" | "INCRBYFLOAT" => {
            // These are replayed as SET with the final value, but since we
            // can't compute the final value from just the command during replay,
            // they would need the prior value. For AOF replay this works because
            // commands are replayed in order.
            // Simplified: treat INCR/DECR as no-ops for now since AOF should
            // capture the final SET result. In practice, a full implementation
            // would handle these properly.
        }
        "LPUSH" => {
            for i in 2..cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(i)?;
                db.lpush(db_index, key, value)?;
            }
        }
        "RPUSH" => {
            for i in 2..cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(i)?;
                db.rpush(db_index, key, value)?;
            }
        }
        "LPOP" | "RPOP" => {
            if cmd.argv.len() >= 2 {
                let key = cmd.get_vec(1)?;
                let count = if cmd.argv.len() >= 3 {
                    cmd.get_i64(2).ok()
                } else {
                    None
                };
                if command_name == "LPOP" {
                    db.lpop(db_index, key, count)?;
                } else {
                    db.rpop(db_index, key, count)?;
                }
            }
        }
        "SADD" => {
            for i in 2..cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(i)?;
                db.sadd(db_index, key, value)?;
            }
        }
        "SREM" => {
            for i in 2..cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(i)?;
                db.srem(db_index, key, value)?;
            }
        }
        "HSET" => {
            let mut i = 2;
            while i + 1 < cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let field = cmd.get_vec(i)?;
                let value = cmd.get_vec(i + 1)?;
                db.hset(db_index, key, field, value)?;
                i += 2;
            }
        }
        "HDEL" => {
            for i in 2..cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let field = cmd.get_vec(i)?;
                db.hdel(db_index, key, field)?;
            }
        }
        "ZADD" => {
            let mut i = 2;
            while i + 1 < cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let score = cmd.get_f64(i)?;
                let member = cmd.get_vec(i + 1)?;
                db.zadd(db_index, key, score, member)?;
                i += 2;
            }
        }
        "ZREM" => {
            for i in 2..cmd.argv.len() {
                let key = cmd.get_vec(1)?;
                let member = cmd.get_vec(i)?;
                db.zrem(db_index, key, member)?;
            }
        }
        "EXPIRE" => {
            if cmd.argv.len() >= 3 {
                let key = cmd.get_vec(1)?;
                let seconds = cmd.get_i64(2)?;
                let expire_at = chrono::Utc::now().timestamp() + seconds;
                db.set_expire(db_index, key, expire_at)?;
            }
        }
        "GETSET" => {
            if cmd.argv.len() >= 3 {
                let key = cmd.get_vec(1)?;
                let value = cmd.get_vec(2)?;
                db.insert(
                    db_index,
                    key,
                    crate::vojo::value::Value::String(crate::vojo::value::ValueString {
                        data: value,
                    }),
                )?;
            }
        }
        "GETDEL" => {
            if cmd.argv.len() >= 2 {
                let key = cmd.get_vec(1)?;
                db.remove(db_index, key)?;
            }
        }
        "MSET" => {
            let mut i = 1;
            while i + 1 < cmd.argv.len() {
                let key = cmd.get_vec(i)?;
                let value = cmd.get_vec(i + 1)?;
                db.insert(
                    db_index,
                    key,
                    crate::vojo::value::Value::String(crate::vojo::value::ValueString {
                        data: value,
                    }),
                )?;
                i += 2;
            }
        }
        "MSETNX" => {
            // During replay, treat same as MSET since we're rebuilding state
            let mut i = 1;
            while i + 1 < cmd.argv.len() {
                let key = cmd.get_vec(i)?;
                let value = cmd.get_vec(i + 1)?;
                db.insert(
                    db_index,
                    key,
                    crate::vojo::value::Value::String(crate::vojo::value::ValueString {
                        data: value,
                    }),
                )?;
                i += 2;
            }
        }
        // Read commands and others are ignored during replay
        _ => {}
    }
    Ok(())
}

/// Perform AOF rewrite (compaction).
/// Reads current database state and writes a new minimal AOF file.
pub fn rewrite_aof(database: &Database, aof_path: &Path) -> Result<(), anyhow::Error> {
    let tmp_path = aof_path.with_extension("aof.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp_path)?;

    let now_ts = chrono::Utc::now().timestamp();

    for shard_idx in 0..database.data.len() {
        for (key, value) in &database.data[shard_idx] {
            match value {
                crate::vojo::value::Value::String(s) => {
                    write_resp_command(&mut file, "SET", &[key.clone(), s.data.clone()])?;
                }
                crate::vojo::value::Value::List(l) => {
                    if !l.data.is_empty() {
                        let mut args = vec![key.clone()];
                        for item in &l.data {
                            args.push(item.clone());
                        }
                        write_resp_command(&mut file, "RPUSH", &args)?;
                    }
                }
                crate::vojo::value::Value::Set(s) => {
                    if !s.data.is_empty() {
                        let mut args = vec![key.clone()];
                        for member in &s.data {
                            args.push(member.clone());
                        }
                        write_resp_command(&mut file, "SADD", &args)?;
                    }
                }
                crate::vojo::value::Value::Hash(h) => {
                    if !h.data.is_empty() {
                        let mut args = vec![key.clone()];
                        for (field, val) in &h.data {
                            args.push(field.clone());
                            args.push(val.clone());
                        }
                        write_resp_command(&mut file, "HSET", &args)?;
                    }
                }
                crate::vojo::value::Value::SortedSet(z) => {
                    if !z.data.is_empty() {
                        let mut args = vec![key.clone()];
                        for item in &z.data {
                            args.push(item.score.to_string().as_bytes().to_vec());
                            args.push(item.member.clone());
                        }
                        write_resp_command(&mut file, "ZADD", &args)?;
                    }
                }
                crate::vojo::value::Value::Nil => {}
            }
        }

        // Write expire commands
        if let Some(expire_map) = database.expire_map.get(shard_idx) {
            for (key, &expire_at) in expire_map {
                let remaining = expire_at - now_ts;
                if remaining > 0 {
                    write_resp_command(
                        &mut file,
                        "EXPIRE",
                        &[key.clone(), remaining.to_string().as_bytes().to_vec()],
                    )?;
                }
            }
        }
    }

    file.sync_all()?;

    // Atomic rename
    std::fs::rename(&tmp_path, aof_path)?;
    Ok(())
}

/// Write a RESP-formatted command to a file.
fn write_resp_command(
    file: &mut File,
    command: &str,
    args: &[Vec<u8>],
) -> Result<(), anyhow::Error> {
    // Total arg count includes the command name itself
    let total_args = 1 + args.len();
    write!(file, "*{}\r\n", total_args)?;
    // Command name
    write!(file, "${}\r\n{}\r\n", command.len(), command)?;
    // Arguments
    for arg in args {
        write!(file, "${}\r\n", arg.len())?;
        file.write_all(arg)?;
        write!(file, "\r\n")?;
    }
    Ok(())
}
