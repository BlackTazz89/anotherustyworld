use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use byteorder::{BigEndian, ReadBytesExt};
use iter_read::IterRead;
use thiserror::Error;

use crate::mem_entry::MemEntry;

#[derive(Error, Debug)]
pub enum BankError {
    #[error("Error while opening bank file")]
    OnOpen(io::Error),
    #[error("IO error while reading bank")]
    Io(io::Error),
}

impl From<io::Error> for BankError {
    fn from(value: io::Error) -> Self {
        BankError::Io(value)
    }
}

pub struct BankReader {}

impl BankReader {
    pub fn read_bank(data_dir: &Path, mem_entry: &mut MemEntry) -> Result<Vec<u8>, BankError> {
        let name = format!("bank{:02x}", mem_entry.bank_id);
        let mut file = File::open(data_dir.join(&name)).map_err(BankError::OnOpen)?;

        file.seek(SeekFrom::Start(mem_entry.bank_offset.into()))?;
        let mut buf = vec![0; mem_entry.packed_size as usize];
        file.read_exact(&mut buf)?;

        if mem_entry.packed_size == mem_entry.size {
            return Ok(buf);
        }

        let mut unpacker = Unpacker::new(IterRead::new(buf.chunks(4).rev().flatten()));
        Ok(unpacker.unpack()?)
    }
}

struct Unpacker<I: Read> {
    reader: I,
    ctx: UnpackContext,
}

#[derive(Default)]
struct UnpackContext {
    crc: u32,
    chk: u32,
    datasize: i32,
}

impl<I: Read> Unpacker<I> {
    fn new(reader: I) -> Self {
        Self {
            reader,
            ctx: UnpackContext::default(),
        }
    }

    fn decode_literal(
        &mut self,
        bit_length: u8,
        additional_length: u8,
        output: &mut Vec<u8>,
    ) -> Result<(), io::Error> {
        let length: u16 = self.get_code(bit_length)? + additional_length as u16 + 1;
        for _ in 0..length {
            let data = self.get_code(8)? as u8;
            output.push(data);
        }
        self.ctx.datasize -= length as i32;
        Ok(())
    }

    fn decode_reference(
        &mut self,
        bit_length: u8,
        length: u16,
        output: &mut Vec<u8>,
    ) -> Result<(), io::Error> {
        let offset = output.len() as u16 - self.get_code(bit_length)?;
        for i in 0..length {
            let data: u8 = output.get((offset + i) as usize).copied().unwrap_or(0u8);
            output.push(data);
        }
        self.ctx.datasize -= length as i32;
        Ok(())
    }

    pub fn unpack(&mut self) -> Result<Vec<u8>, io::Error> {
        let ctx = &mut self.ctx;
        ctx.datasize = self.reader.read_i32::<BigEndian>()?;
        ctx.crc = self.reader.read_u32::<BigEndian>()?;
        ctx.chk = self.reader.read_u32::<BigEndian>()?;
        ctx.crc ^= ctx.chk;

        let mut output = vec![];
        loop {
            if self.ctx.datasize <= 0 {
                break;
            }

            if self.get_next_bit()? == 0 {
                if self.get_next_bit()? == 0 {
                    self.decode_literal(3, 0, &mut output)?
                } else {
                    self.decode_reference(8, 2, &mut output)?
                }
            } else {
                let code = self.get_code(2)?;
                if code == 3 {
                    self.decode_literal(8, 8, &mut output)?;
                } else if code < 2 {
                    self.decode_reference(code as u8 + 9, code + 3, &mut output)?;
                } else {
                    let length = self.get_code(8)? + 1;
                    self.decode_reference(12, length, &mut output)?;
                }
            }
        }

        output.reverse();
        Ok(output)
    }

    fn get_code(&mut self, bit_length: u8) -> Result<u16, io::Error> {
        let mut code: u16 = 0;
        for _ in 0..bit_length {
            code <<= 1;
            code |= self.get_next_bit()? as u16;
        }
        Ok(code)
    }

    fn get_next_bit(&mut self) -> Result<u8, io::Error> {
        let mut lsb = self.rcr(false);
        if self.ctx.chk == 0 {
            self.ctx.chk = self.reader.read_u32::<BigEndian>()?;
            self.ctx.crc ^= self.ctx.chk;
            lsb = self.rcr(true);
        }
        Ok(lsb)
    }

    fn rcr(&mut self, carry: bool) -> u8 {
        let lsb: u8 = (self.ctx.chk & 1) as u8;
        self.ctx.chk >>= 1;
        if carry {
            self.ctx.chk |= 0x80000000;
        }
        lsb
    }
}
