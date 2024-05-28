use super::{fs_writer::MyReader, lib::Database};
use bincode::config;
use std::fs::OpenOptions;
use tokio::time::Instant;
pub async fn load_rdb(file_path: String) -> Result<Database, anyhow::Error> {
    info!("Rdb file is loading ,file path is: {}", file_path);
    let now = Instant::now();
    let file = OpenOptions::new().read(true).open(file_path.clone())?;
    let config = config::standard();
    let my_reader = MyReader(file);
    let database: Database = bincode::decode_from_reader(my_reader, config.clone())?;
    let key_len = database.data[0].len();
    info!(
        "Rdb file has been loaded,keys count is {},total time cost {}ms",
        key_len,
        now.elapsed().as_millis()
    );
    Ok(database)
}
