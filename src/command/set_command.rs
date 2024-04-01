use anyhow::{anyhow, ensure};

use crate::database::lib::Database;
use crate::parser::response::Response;
use crate::util::common_utils::mstime;
use crate::vojo::parsered_command::ParsedCommand;
use crate::vojo::value::Value;
use crate::vojo::value::ValueList;
use crate::vojo::value::ValueString;
pub fn sadd(
    parser: ParsedCommand,
    db: &mut Database,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    ensure!(parser.argv.len() >= 3, "InvalidArgument");
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
