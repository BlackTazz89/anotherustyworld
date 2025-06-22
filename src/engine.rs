use std::path::PathBuf;

use thiserror::Error;
use winit::{event_loop::EventLoop, window::WindowBuilder};

use crate::{
    execution_context::ExecutionContext,
    loaded::{LoadedAsset, LoadedPart},
    parts::GamePart,
    renderer::{Renderer, SCALED_H, SCALED_W},
    resource::{ResourceError, ResourceRegistry},
    sys_event_handler::SysEventHandler,
    video::Video,
    vm::{Vm, VmError},
};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Resource registry error")]
    ResourceError(ResourceError),
    #[error("Unexpected error in VM execution")]
    VmError(VmError),
}

impl From<ResourceError> for EngineError {
    fn from(value: ResourceError) -> Self {
        EngineError::ResourceError(value)
    }
}

impl From<VmError> for EngineError {
    fn from(value: VmError) -> Self {
        EngineError::VmError(value)
    }
}

pub struct Engine {}

impl Engine {
    pub fn run(data_dir: PathBuf) -> Result<(), EngineError> {
        let event_loop = EventLoop::new().unwrap();
        let window = WindowBuilder::new()
            .with_title("Another Rusty World")
            .with_inner_size(winit::dpi::LogicalSize::new(
                SCALED_W as u32,
                SCALED_H as u32,
            ))
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();

        let mut _sys_event_handler = SysEventHandler::new(event_loop);
        let mut resource = ResourceRegistry::new(data_dir);
        let mut video = Video::new(Renderer::new(window));
        let mut vm = Vm::default();

        resource.read_entries()?;
        let mut context = ExecutionContext::new(
            LoadedPart::default(),
            LoadedAsset::default(),
            Some(GamePart::Two),
            &mut resource,
            &mut video,
        );

        loop {
            Self::update_part(&mut context, &mut vm)?;
            vm.check_channel_requests()?;
            vm.host_frame(&mut context)?;
        }
    }

    fn update_part(context: &mut ExecutionContext, vm: &mut Vm) -> Result<(), EngineError> {
        if let Some(part_id) = context.part_to_load {
            vm.init_part()?;

            let loaded_part = context.resource.setup_part(part_id)?;
            if let Some(polygon) = &loaded_part.polygon {
                context.video.copy_bg(polygon.get_ref());
            }
            context.loaded_part = loaded_part;
            context.loaded_asset = LoadedAsset::default();
            context.part_to_load = None;
        }
        Ok(())
    }
}
