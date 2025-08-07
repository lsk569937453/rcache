use anyhow::{anyhow, ensure};

use crate::parser::response::Response;

use crate::vojo::parsered_command::ParsedCommand;

use crate::database::lib::DatabaseHolder;

pub fn sadd(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
    let mut db = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;
    let mut count = 0;
    for i in 2..parser.argv.len() {
        let val = parser.get_vec(i)?;
        if db.sadd(db_index, key.clone(), val)? {
            count += 1;
        }
    }

    Ok(Response::Integer(count))
}
pub fn smembers(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(
        parser.argv.len() == 2,
        "InvalidArgument: wrong number of arguments for 'smembers' command"
    );

    let mut database = database_lock
        .database_lock
        .lock()
        .map_err(|e| anyhow!("{}", e))?;
    let key = parser.get_vec(1)?;

    match database.get(db_index, key)? {
        Some(value) => {
            ensure!(
                value.is_set(),
                "WRONGTYPE Operation against a key holding the wrong kind of value"
            );
            let set_value = value.to_value_set()?;
            let members_as_responses: Vec<Response> = set_value
                .data
                .iter()
                .map(|member| Response::Data(member.clone()))
                .collect();

            Ok(Response::Array(members_as_responses))
        }
        None => Ok(Response::Array(Vec::new())),
    }
}
