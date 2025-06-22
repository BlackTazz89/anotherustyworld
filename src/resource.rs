use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
};

use crate::{
    bank::{BankError, BankReader},
    loaded::{LoadedPart, LoadedPartError},
    mem_entry::{MemEntry, MemEntryError},
    parts::{GamePart, SEGMENT_IDX_BY_PART, Segment},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResourceError {
    #[error("Error opening memlist file")]
    MemListOpen(io::Error),
    #[error("Error while processing bank data")]
    BankError(BankError),
    #[error("Error while creating MemEntry")]
    MemEntryError(MemEntryError),
    #[error("Error while loading game part")]
    LoadedPartError(LoadedPartError),
}

impl From<MemEntryError> for ResourceError {
    fn from(value: MemEntryError) -> Self {
        ResourceError::MemEntryError(value)
    }
}

impl From<BankError> for ResourceError {
    fn from(value: BankError) -> Self {
        ResourceError::BankError(value)
    }
}

impl From<LoadedPartError> for ResourceError {
    fn from(value: LoadedPartError) -> Self {
        ResourceError::LoadedPartError(value)
    }
}

#[derive(Default)]
pub struct ResourceRegistry {
    data_dir: PathBuf,
    pub mem_list: Vec<MemEntry>,
}

impl ResourceRegistry {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            ..Default::default()
        }
    }

    pub fn read_entries(&mut self) -> Result<(), ResourceError> {
        let file_path = self.data_dir.join("memlist.bin");
        let file = File::open(file_path).map_err(ResourceError::MemListOpen)?;
        let mut reader = BufReader::new(file);

        for _ in 0..=145 {
            let mem_entry = MemEntry::from_reader(&mut reader)?;
            self.mem_list.push(mem_entry);
        }

        Ok(())
    }

    pub fn load_entry(&mut self, index: usize) -> Result<Vec<u8>, ResourceError> {
        let entry = &mut self.mem_list[index];
        Ok(BankReader::read_bank(&self.data_dir, entry)?)
    }

    pub fn setup_part(&mut self, game_part: GamePart) -> Result<LoadedPart, ResourceError> {
        let part_idx = game_part as usize - GamePart::One as usize;

        let segment_data: HashMap<Segment, Vec<u8>> = [
            Segment::Palette,
            Segment::Bytecode,
            Segment::PolyCinematic,
            Segment::Polygon,
        ]
        .map(|segment| (segment, SEGMENT_IDX_BY_PART[part_idx][segment as usize]))
        .into_iter()
        .filter(|(_, idx)| *idx != 0)
        .try_fold(
            HashMap::new(),
            |mut map, (segment, idx)| -> Result<HashMap<Segment, Vec<u8>>, ResourceError> {
                map.insert(segment, self.load_entry(idx)?);
                Ok(map)
            },
        )?;

        Ok(LoadedPart::from(segment_data)?)

        //if let Some(video_seg) = self.loaded_segments.get(&Segment::Polygon) {
        //    video.copy_bg(&self.mem_list[*video_seg].data);
        //}
    }
}
