use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub persistence: PersistenceConfig,
    pub memory: MemoryConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u32,
    #[serde(default)]
    pub cluster_enabled: bool,
    #[serde(default = "default_cluster_node_timeout")]
    pub cluster_node_timeout: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PersistenceConfig {
    pub rdb_path: Option<String>,
    #[serde(default)]
    pub aof_enabled: bool,
    #[serde(default = "default_aof_path")]
    pub aof_path: String,
    #[serde(default = "default_aof_sync_policy")]
    pub aof_sync_policy: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MemoryConfig {
    #[serde(default = "default_maxmemory")]
    pub maxmemory: String,
}

fn default_port() -> u32 {
    6370
}
fn default_cluster_node_timeout() -> u64 {
    15000
}
fn default_aof_path() -> String {
    "rcache.aof".to_string()
}
fn default_aof_sync_policy() -> String {
    "everysec".to_string()
}
fn default_maxmemory() -> String {
    "0".to_string()
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Parse maxmemory string (e.g. "256mb", "1gb", "0") into bytes.
    /// Returns 0 for unlimited.
    pub fn max_memory_bytes(&self) -> usize {
        let s = self.memory.maxmemory.trim();
        if s == "0" {
            return 0;
        }
        // Use byte-unit crate
        match s.parse::<byte_unit::Byte>() {
            Ok(b) => b.as_u64() as usize,
            Err(_) => 0,
        }
    }

    /// Get the RDB file path as an absolute or relative PathBuf.
    /// Returns None if not configured.
    pub fn rdb_path_buf(&self) -> Option<PathBuf> {
        self.persistence.rdb_path.as_ref().map(PathBuf::from)
    }

    pub fn aof_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.persistence.aof_path)
    }
}
