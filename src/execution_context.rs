use std::time::Instant;

use crate::{
    loaded::{LoadedAsset, LoadedPart},
    parts::GamePart,
    resource::ResourceRegistry,
    video::Video,
};

pub struct ExecutionContext<'a> {
    pub loaded_part: LoadedPart,
    pub loaded_asset: LoadedAsset,
    pub part_to_load: Option<GamePart>,
    pub resource: &'a mut ResourceRegistry,
    pub video: &'a mut Video,
    pub last_rendering: Instant,
}

impl<'a> ExecutionContext<'a> {
    pub fn new(
        loaded_part: LoadedPart,
        loaded_asset: LoadedAsset,
        part_to_load: Option<GamePart>,
        resource: &'a mut ResourceRegistry,
        video: &'a mut Video,
    ) -> Self {
        Self {
            loaded_part,
            loaded_asset,
            part_to_load,
            resource,
            video,
            last_rendering: Instant::now(),
        }
    }
}
