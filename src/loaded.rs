use std::{collections::HashMap, io::Cursor};

use thiserror::Error;

use crate::parts::Segment::{self, Bytecode, Palette, PolyCinematic, Polygon};

macro_rules! extract_required {
    ($map:expr, $segment:expr) => {
        $map.remove(&$segment)
            .ok_or(LoadedPartError::MissingSegment($segment))?
    };
}

#[derive(Error, Debug)]
pub enum LoadedPartError {
    #[error("Missing segment")]
    MissingSegment(Segment),
}

#[derive(Default)]
pub struct LoadedPart {
    pub bytecode: Cursor<Vec<u8>>,
    pub palette: Cursor<Vec<u8>>,
    pub cinematic: Cursor<Vec<u8>>,
    pub polygon: Option<Cursor<Vec<u8>>>,
}

impl LoadedPart {
    pub fn from(mut segment_data: HashMap<Segment, Vec<u8>>) -> Result<Self, LoadedPartError> {
        let bytecode = extract_required!(segment_data, Bytecode);
        let palette = extract_required!(segment_data, Palette);
        let cinematic = extract_required!(segment_data, PolyCinematic);
        let polygon = segment_data.remove(&Polygon);
        let loaded_part = Self {
            bytecode: Cursor::new(bytecode),
            palette: Cursor::new(palette),
            cinematic: Cursor::new(cinematic),
            polygon: polygon.map(Cursor::new),
        };
        Ok(loaded_part)
    }
}

type MemEntryIndex = usize;
#[derive(Default)]
pub struct LoadedAsset {
    pub assets: HashMap<MemEntryIndex, Vec<u8>>,
}
