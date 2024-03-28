use crate::anyhow;
use crate::command::parser::{Argument, ParsedCommand};

pub struct Request {}

impl Request {
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
        let argc = match argco {
            Some(i) => i,
            None => 0,
        };
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
        Ok((ParsedCommand::new(input, argv), pos))
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
        } else if c < '0' || c > '9' {
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
