use crate::database::lib::DatabaseHolder;
use crate::parser::response::Response;
use crate::vojo::parsered_command::ParsedCommand;
use anyhow::anyhow;

/// HELLO [protover [AUTH username password] [SETNAME clientname]]
/// Switch to RESP3 and optionally authenticate
///
/// This is a simplified implementation that accepts the HELLO command
/// and returns a basic RESP3 response
pub fn hello(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    info!("HELLO: handling RESP3 handshake");

    // Parse protocol version if provided
    if cmd.argv.len() >= 2 {
        let protover = cmd.get_str(1)?;
        info!("HELLO: protocol version requested: {}", protover);
    }

    // Return a basic RESP3 hello response
    // Format: map with server info
    Ok(Response::Array(vec![
        Response::Data(b"server".to_vec()),
        Response::Data(b"redis".to_vec()),
        Response::Data(b"version".to_vec()),
        Response::Data(b"999.999.999".to_vec()),
        Response::Data(b"proto".to_vec()),
        Response::Data(if cmd.argv.len() >= 2 {
            cmd.get_slice(1)?.to_vec()
        } else {
            b"3".to_vec()
        }),
        Response::Data(b"id".to_vec()),
        Response::Integer(1),
        Response::Data(b"mode".to_vec()),
        Response::Data(b"standalone".to_vec()),
        Response::Data(b"role".to_vec()),
        Response::Data(b"master".to_vec()),
    ]))
}

/// AUTH [username] password
/// Authenticate the connection
///
/// For simplicity, we accept any authentication without actually checking
pub fn auth(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() < 2 {
        error!("AUTH: wrong number of arguments (expected >= 2, got {})", cmd.argv.len());
        return Err(anyhow!("ERR wrong number of arguments for 'AUTH' command"));
    }

    if cmd.argv.len() >= 3 {
        let username = cmd.get_str(1)?;
        let _password = cmd.get_str(2)?;
        info!("AUTH: username='{}', password='***'", username);
    } else {
        let _password = cmd.get_str(1)?;
        info!("AUTH: password='***'");
    }

    // Accept any authentication
    Ok(Response::Status("OK".to_owned()))
}

/// SELECT index
/// Select the database with the specified index
///
/// For simplicity, we acknowledge the command but don't actually switch databases
pub fn select_db(cmd: ParsedCommand) -> Result<Response, anyhow::Error> {
    if cmd.argv.len() != 2 {
        error!("SELECT: wrong number of arguments (expected 2, got {})", cmd.argv.len());
        return Err(anyhow!("ERR wrong number of arguments for 'SELECT' command"));
    }

    let db_index = cmd.get_str(1)?;
    info!("SELECT: database index '{}'", db_index);

    // Parse and validate the index
    let index: u8 = db_index.parse()
        .map_err(|_| anyhow!("ERR invalid DB index"))?;

    if index > 15 {
        return Err(anyhow!("ERR DB index is out of range"));
    }

    // Accept the SELECT command
    Ok(Response::Status("OK".to_owned()))
}

/// INFO [section]
/// Return server information and statistics.
/// Supported sections: server, keyspace, all (default)
pub fn info(
    cmd: ParsedCommand,
    database_lock: &mut DatabaseHolder,
    db_index: usize,
) -> Result<Response, anyhow::Error> {
    let db = database_lock.database_lock.lock().map_err(|e| anyhow!("{}", e))?;

    let section = if cmd.argv.len() >= 2 {
        cmd.get_str(1)?.to_lowercase()
    } else {
        "all".to_string()
    };

    let info_text = match section.as_str() {
        "server" => db.node_info.build_server_section(),
        "keyspace" => crate::database::info::NodeInfo::build_keyspace_section(&db, db_index),
        _ => db.node_info.build_info(&db, db_index),
    };

    Ok(Response::Data(info_text.as_bytes().to_vec()))
}
