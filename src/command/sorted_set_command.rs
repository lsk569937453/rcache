use std::collections::LinkedList;

use anyhow::{anyhow, ensure};

use crate::database::lib::Database;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueList;
use crate::vojo::value::ValueString;
pub fn zadd(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 2, "InvalidArgument");
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