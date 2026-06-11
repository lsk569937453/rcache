use super::lib::Database;
use rkyv::rancor::Error as RkyvError;
use tokio::time::Instant;
pub async fn load_rdb(file_path: String) -> Result<Database, anyhow::Error> {
    info!("Rdb file is loading ,file path is: {}", file_path);
    let now = Instant::now();
    let bytes = std::fs::read(&file_path)?;
    let database: Database = rkyv::from_bytes::<Database, RkyvError>(&bytes)
        .map_err(|e| anyhow!("rkyv deserialize: {}", e))?;
    let key_len = database.data[0].len();
    info!(
        "Rdb file has been loaded,keys count is {},total time cost {}ms",
        key_len,
        now.elapsed().as_millis()
    );
    Ok(database)
}
