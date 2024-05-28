use bincode::error::DecodeError;
use bincode::error::EncodeError;
use bincode::{de::read::Reader, enc::write::Writer};
use std::io::prelude::*;
use std::io::Read;

use std::{fs::File, io::Write};
pub struct MyWriter(pub File);
impl Writer for MyWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.0.write_all(bytes);

        Ok(())
    }
}
pub struct MyReader(pub File);
impl Reader for MyReader {
    /// Fill the given `bytes` argument with values. Exactly the length of the given slice must be filled, or else an error must be returned.
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), DecodeError> {
        self.0
            .read(bytes)
            .map_err(|e| DecodeError::OtherString(e.to_string()))?;
        Ok(())
    }
}
