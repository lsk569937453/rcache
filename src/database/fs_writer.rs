use bincode::enc::write::Writer;
use bincode::error::EncodeError;
use std::{fs::File, io::Write};
pub struct MyWriter(pub File);
impl Writer for MyWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.0.write_all(bytes);
        Ok(())
    }
}
