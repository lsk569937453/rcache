use crate::vojo::parsered_command::ParsedCommand;

use crate::parser::response::Response;

/// PING command
///
/// Returns PONG if no argument is provided.
/// Returns the argument if one is provided.
pub fn ping(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    // Redis PING supports an optional message argument
    // PING returns "PONG"
    // PING "hello" returns "hello"
    if cmd.argv.len() > 1 {
        let message = cmd.get_str(1)?;
        Ok(Response::Data(message.as_bytes().to_vec()))
    } else {
        Ok(Response::Status("PONG".to_owned()))
    }
}
