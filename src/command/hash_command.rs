use anyhow::{anyhow, ensure};

use crate::database::lib::Database;
use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;

use crate::vojo::value::Value;
use crate::vojo::value::ValueList;
use crate::vojo::value::ValueString;
pub  fn hset(
    parser: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() > 3, "InvalidArgument");
    let mut db = database_lock.database_lock.lock().map_err(|e|anyhow!("{}",e))?;
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
