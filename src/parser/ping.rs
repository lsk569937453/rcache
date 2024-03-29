use crate::vojo::parsered_command::ParsedCommand;

use crate::parser::response::Response;
pub fn ping(parser: ParsedCommand) -> Result<Response, anyhow::Error> {
    Ok(Response::Status("PONG".to_owned()))
}
