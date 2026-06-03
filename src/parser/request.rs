use crate::anyhow;
use crate::vojo::parsered_command::{Argument, ParsedCommand};

pub struct Request {}

impl Request {
    /// Parse all commands from a buffer (for pipeline support)
    /// Returns a vector of parsed commands
    pub fn parse_all(input: &[u8]) -> Result<Vec<ParsedCommand>, anyhow::Error> {
        let mut commands = Vec::new();
        let mut pos = 0;

        while pos < input.len() {
            // Skip any leading \r\n
            while pos < input.len() && input[pos] as char == '\r' {
                if pos + 1 < input.len() && input[pos + 1] as char == '\n' {
                    pos += 2;
                } else {
                    break;
                }
            }

            if pos >= input.len() {
                break;
            }

            // Expect '*' for array start
            if input[pos] as char != '*' {
                return Err(anyhow!(format!(
                    "expected '*', got '{}'",
                    input[pos] as char
                )));
            }

            let (parsed_cmd, consumed) = Self::parse_buf_with_offset(input, pos)?;
            commands.push(parsed_cmd);
            pos += consumed;
        }

        Ok(commands)
    }

    /// Parse a single command with an offset for position tracking
    /// Used by parse_all to maintain correct positions in ParsedCommand
    /// Takes the full input and the starting position (offset) to parse from
    fn parse_buf_with_offset(input: &[u8], offset: usize) -> Result<(ParsedCommand, usize), anyhow::Error> {
        let mut pos = offset;
        let start_pos = offset; // Remember where we started

        // Skip leading \r\n at current position
        while pos < input.len() && input[pos] as char == '\r' {
            if pos + 1 < input.len() {
                if input[pos + 1] as char != '\n' {
                    return Err(anyhow!(format!(
                        "expected \\r\\n separator, got \
                         \\r{}",
                        input[pos + 1] as char
                    )));
                }
                pos += 2;
            } else {
                return Err(anyhow!("Incomplete request"));
            }
        }

        if pos >= input.len() {
            return Err(anyhow!("Incomplete request"));
        }

        if input[pos] as char != '*' {
            return Err(anyhow!(format!(
                "expected '*', got '{}' at pos={}",
                input[pos] as char, pos
            )));
        }
        pos += 1;
        let len = input.len();
        let (argco, intlen) = parse_int(&input[pos..len], len - pos, "multibulk")?;
        let argc = argco.unwrap_or_default();
        pos += intlen;
        if argc > 1024 * 1024 {
            return Err(anyhow!("invalid multibulk length".to_owned(),));
        }
        let mut argv = Vec::new();
        for i in 0..argc {
            if input.len() == pos {
                return Err(anyhow!("Incomplete request"));
            }
            if input[pos] as char != '$' {
                return Err(anyhow!(format!(
                    "expected '$', got '{}'",
                    input[pos] as char
                )));
            }
            pos += 1;
            let (argleno, arglenlen) = parse_int(&input[pos..len], len - pos, "bulk")?;
            let arglen = match argleno {
                Some(i) => i,
                None => return Err(anyhow!("invalid bulk length".to_owned())),
            };
            if arglen > 512 * 1024 * 1024 {
                return Err(anyhow!("invalid bulk length".to_owned()));
            }
            pos += arglenlen;
            // Store absolute position (from beginning of input)
            let arg = Argument { pos, len: arglen };
            argv.push(arg);
            pos += arglen + 2;
            if pos > len || (pos == len && i != argc - 1) {
                return Err(anyhow!("Incomplete request"));
            }
        }
        // Return ParsedCommand with full input data and consumed bytes (from start_pos)
        Ok((ParsedCommand::new(input.to_vec(), argv), pos - start_pos))
    }

    /// Parse a single command (original method, used for single command parsing)
    pub fn parse_buf(input: &[u8]) -> Result<(ParsedCommand, usize), anyhow::Error> {
        let mut pos = 0;
        while input.len() > pos && input[pos] as char == '\r' {
            if pos + 1 < input.len() {
                if input[pos + 1] as char != '\n' {
                    return Err(anyhow!(format!(
                        "expected \\r\\n separator, got \
                         \\r{}",
                        input[pos + 1] as char
                    )));
                }
                pos += 2;
            } else {
                return Err(anyhow!("Incomplete request"));
            }
        }
        if pos >= input.len() {
            return Err(anyhow!("Incomplete request"));
        }
        if input[pos] as char != '*' {
            return Err(anyhow!(format!(
                "expected '*', got '{}'",
                input[pos] as char
            )));
        }
        pos += 1;
        let len = input.len();
        let (argco, intlen) = parse_int(&input[pos..len], len - pos, "multibulk")?;
        let argc = argco.unwrap_or_default();
        pos += intlen;
        if argc > 1024 * 1024 {
            return Err(anyhow!("invalid multibulk length".to_owned(),));
        }
        let mut argv = Vec::new();
        for i in 0..argc {
            if input.len() == pos {
                return Err(anyhow!("Incomplete request"));
            }
            if input[pos] as char != '$' {
                return Err(anyhow!(format!(
                    "expected '$', got '{}'",
                    input[pos] as char
                )));
            }
            pos += 1;
            let (argleno, arglenlen) = parse_int(&input[pos..len], len - pos, "bulk")?;
            let arglen = match argleno {
                Some(i) => i,
                None => return Err(anyhow!("invalid bulk length".to_owned())),
            };
            if arglen > 512 * 1024 * 1024 {
                return Err(anyhow!("invalid bulk length".to_owned()));
            }
            pos += arglenlen;
            let arg = Argument { pos, len: arglen };
            argv.push(arg);
            pos += arglen + 2;
            if pos > len || (pos == len && i != argc - 1) {
                return Err(anyhow!("Incomplete request"));
            }
        }
        Ok((ParsedCommand::new(input.to_vec(), argv), pos))
    }
}
fn parse_int(
    input: &[u8],
    len: usize,
    name: &str,
) -> Result<(Option<usize>, usize), anyhow::Error> {
    if input.is_empty() {
        return Err(anyhow!("Incomplete request"));
    }
    let mut i = 0;
    let mut argc = 0;
    let mut argco = None;
    while input[i] as char != '\r' {
        let c = input[i] as char;
        if argc == 0 && c == '-' {
            while input[i] as char != '\r' {
                i += 1;
            }
            argco = None;
            break;
        } else if !c.is_ascii_digit() {
            return Err(anyhow!(format!("invalid {} length", name)));
        }
        argc *= 10;
        argc += input[i] as usize - '0' as usize;
        i += 1;
        if i == len {
            return Err(anyhow!("Incomplete request"));
        }
        argco = Some(argc);
    }
    i += 1;
    if i == len {
        return Err(anyhow!("Incomplete request"));
    }
    if input[i] as char != '\n' {
        return Err(anyhow!(format!(
            "expected \\r\\n separator, got \\r{}",
            input[i] as char
        )));
    }

    Ok((argco, i + 1))
}
