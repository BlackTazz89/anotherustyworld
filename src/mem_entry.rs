use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemEntryError {
    #[error("Error while reading the underlying stream")]
    Io(io::Error),
    #[error("Invalid resource status: {0}")]
    InvalidState(u8),
    #[error("Invalid resource type: {0}")]
    InvalidType(u8),
}

impl From<io::Error> for MemEntryError {
    fn from(value: io::Error) -> Self {
        MemEntryError::Io(value)
    }
}

#[derive(Debug)]
pub struct MemEntry {
    pub bank_id: u8,
    pub bank_offset: u32,
    pub packed_size: u16,
    pub size: u16,
}

impl MemEntry {
    pub fn from_reader<R: ReadBytesExt>(reader: &mut R) -> Result<Self, MemEntryError> {
        reader.read_u8()?;
        reader.read_u8()?;
        reader.read_u16::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?;
        reader.read_u8()?;
        let bank_id = reader.read_u8()?;
        let bank_offset = reader.read_u32::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?;
        let packed_size = reader.read_u16::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?;
        let size = reader.read_u16::<BigEndian>()?;

        let mem_entry = MemEntry {
            bank_id,
            bank_offset,
            packed_size,
            size,
        };
        Ok(mem_entry)
    }
}
