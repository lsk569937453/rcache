use bincode::{Decode, Encode};
use chrono::Utc;

use super::lib::Database;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub struct NodeInfo {
    replication: Role,
    pub start_time: i64,
}
impl Default for NodeInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeInfo {
    pub fn new() -> Self {
        let master_info = MasterInfo::new();
        NodeInfo {
            replication: Role::Master(master_info),
            start_time: Utc::now().timestamp(),
        }
    }

    pub fn build_info(&self, db: &Database, db_index: usize) -> String {
        let mut sections = Vec::new();
        sections.push(self.build_server_section());
        sections.push(Self::build_keyspace_section(db, db_index));
        sections.join("\r\n")
    }

    pub fn build_server_section(&self) -> String {
        let uptime = Utc::now().timestamp() - self.start_time;
        let role = match &self.replication {
            Role::Master(_) => "master",
            Role::Slave(_) => "slave",
        };
        format!(
            "# Server\r\nredis_version:{}\r\nredis_mode:standalone\r\nuptime_in_seconds:{}\r\nrole:{}",
            VERSION, uptime, role
        )
    }

    pub fn build_keyspace_section(db: &Database, db_index: usize) -> String {
        let keys = db.data.get(db_index).map(|m| m.len()).unwrap_or(0);
        let expires = db.expire_map.get(db_index).map(|m| m.len()).unwrap_or(0);
        format!(
            "# Keyspace\r\ndb{}:keys={},expires={}",
            db_index, keys, expires
        )
    }
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]

pub enum Role {
    Slave(SlaveInfo),
    Master(MasterInfo),
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub struct SlaveInfo {
    pub master_host: String,
    pub master_port: i32,
    pub master_link_status: String,
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub struct MasterInfo {
    pub connected_slaves: i32,
    pub slaves: Vec<NestedSlaveInfo>,
}
impl MasterInfo {
    pub fn new() -> Self {
        MasterInfo {
            connected_slaves: 0,
            slaves: Vec::new(),
        }
    }
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub struct NestedSlaveInfo {
    pub ip: String,
    pub port: i32,
    pub status: Status,
    pub offset: u128,
    pub lag: i32,
}
#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub enum Status {
    Online,
    OffLine,
}
