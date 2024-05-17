pub struct NodeInfo {
    replication: Role,
}
impl NodeInfo {
    pub fn new() -> Self {
        let master_info = MasterInfo::new();
        NodeInfo {
            replication: Role::Master(master_info),
        }
    }
}
pub enum Role {
    Slave(SlaveInfo),
    Master(MasterInfo),
}
pub struct SlaveInfo {
    pub master_host: String,
    pub master_port: i32,
    pub master_link_status: String,
}
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
pub struct NestedSlaveInfo {
    pub ip: String,
    pub port: i32,
    pub status: Status,
    pub offset: u128,
    pub lag: i32,
}
pub enum Status {
    Online,
    OffLine,
}
