use crate::command::parser::ParsedCommand;

use crate::command::response::Response;
pub fn ping(parser: ParsedCommand) -> Result<Response, anyhow::Error> {
    Ok(Response::Nil)
}
