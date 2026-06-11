use rkyv::{Archive, Deserialize, Serialize};
use chrono::Utc;

use super::lib::Database;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
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

    pub fn build_info_with_memory(
        &self,
        db: &Database,
        db_index: usize,
        used_memory: usize,
        max_memory: usize,
    ) -> String {
        let mut sections = Vec::new();
        sections.push(self.build_server_section());
        sections.push(Self::build_memory_section(used_memory, max_memory));
        sections.push(Self::build_keyspace_section(db, db_index));
        sections.join("\r\n")
    }

    pub fn build_memory_section(used_memory: usize, max_memory: usize) -> String {
        let max_str = if max_memory == 0 {
            "unlimited".to_string()
        } else {
            format!("{}bytes", max_memory)
        };
        format!(
            "# Memory\r\nused_memory:{}\r\nmaxmemory:{}",
            used_memory, max_str
        )
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
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub enum Role {
    Slave(SlaveInfo),
    Master(MasterInfo),
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct SlaveInfo {
    pub master_host: String,
    pub master_port: i32,
    pub master_link_status: String,
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
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
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct NestedSlaveInfo {
    pub ip: String,
    pub port: i32,
    pub status: Status,
    pub offset: u128,
    pub lag: i32,
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub enum Status {
    Online,
    OffLine,
}
