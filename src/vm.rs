use std::{
    io::{self, Seek, SeekFrom},
    thread,
    time::Duration,
};

use byteorder::{BigEndian, ReadBytesExt};
use log::debug;
use rand::random;
use thiserror::Error;

use crate::{
    channel::{Channel, ProcessCounter, State},
    execution_context::ExecutionContext,
    loaded::LoadedAsset,
    opcodes::OPCODE_TABLE,
    resource::ResourceError,
    shapes::Point,
    video::{PageId, PaletteRequest, VideoError},
};

const NUM_CHANNELS: usize = 64;
const NUM_VARIABLES: usize = 256;
const VM_VARIABLE_SCROLL_Y: usize = 0xF9;
const VM_VARIABLE_PAUSE_SLICES: usize = 0xFF;

#[derive(Error, Debug)]
pub enum VmError {
    #[error("IO error reading underlying stream")]
    Io(io::Error),
    #[error("Missing polygon segment")]
    MissingPolygonSegment,
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Video error")]
    VideoError(VideoError),
    #[error("Resource error")]
    ResourceError(ResourceError),
}

impl From<io::Error> for VmError {
    fn from(value: io::Error) -> Self {
        VmError::Io(value)
    }
}

impl From<ResourceError> for VmError {
    fn from(value: ResourceError) -> Self {
        VmError::ResourceError(value)
    }
}

impl From<VideoError> for VmError {
    fn from(value: VideoError) -> Self {
        VmError::VideoError(value)
    }
}

pub struct Vm {
    variables: [i16; NUM_VARIABLES],
    channels: [Channel; NUM_CHANNELS],
    running_channel_id: usize,
    stack: Vec<u64>,
}

impl Default for Vm {
    fn default() -> Self {
        let mut variables = [0; NUM_VARIABLES];
        variables[0x54] = 0x81;
        variables[0x3C] = random::<i16>();
        variables[0xBC] = 0x10;
        variables[0xC6] = 0x80;
        variables[0xF2] = 4000;
        variables[0xDC] = 33;
        let channels = [Channel::default(); NUM_CHANNELS];
        Self {
            variables,
            channels,
            running_channel_id: 0,
            stack: Vec::default(),
        }
    }
}

impl Vm {
    pub fn init_part(&mut self) -> Result<(), VmError> {
        self.variables[0xE4] = 0x14;
        self.channels.iter_mut().for_each(Channel::reset);
        self.channels[0].pc = ProcessCounter::Valid(0);
        Ok(())
    }

    pub fn check_channel_requests(&mut self) -> Result<(), VmError> {
        for channel_id in 0..NUM_CHANNELS {
            let channel = &mut self.channels[channel_id];
            if let Some(set_vec) = channel.pending_setvec {
                channel.pc = ProcessCounter::Valid(set_vec);
                channel.pending_setvec = None;
            }
        }

        Ok(())
    }

    pub fn host_frame(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        for channel_id in 0..NUM_CHANNELS {
            if self.channels[channel_id].state == State::Dead {
                continue;
            }

            if let ProcessCounter::Valid(pc) = self.channels[channel_id].pc {
                self.stack.clear();
                self.run_channel(channel_id, pc, context)?
            }
        }
        Ok(())
    }

    fn run_channel(
        &mut self,
        channel_id: usize,
        channel_pc: usize,
        context: &mut ExecutionContext,
    ) -> Result<(), VmError> {
        debug!(
            "run_channel: invoked. channel_id {} channel_pc {}",
            channel_id, channel_pc
        );
        context
            .loaded_part
            .bytecode
            .seek(SeekFrom::Start(channel_pc as u64))?;

        self.running_channel_id = channel_id;
        self.channels[channel_id].state = State::Running;
        loop {
            let opcode = context.loaded_part.bytecode.read_u8()?;
            match opcode {
                opcode if opcode & 0x80 != 0 => self.draw_background(opcode, context)?,
                opcode if opcode & 0x40 != 0 => self.draw_sprite(opcode, context)?,
                _ => OPCODE_TABLE[opcode as usize](self, context)?,
            };

            if self.channels[channel_id].state == State::Yielding {
                self.channels[channel_id].state = State::Ready;
                break;
            }

            if self.channels[channel_id].state == State::Dead {
                self.channels[channel_id].pc = ProcessCounter::Invalid;
                break;
            }
        }

        self.channels[channel_id].pc = context.loaded_part.bytecode.position().into();
        Ok(())
    }

    pub fn op_mov_const(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let variable_id = bytecode.read_u8()? as usize;
        let value = bytecode.read_u16::<BigEndian>()? as i16;
        self.variables[variable_id] = value;
        Ok(())
    }

    pub fn op_mov(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_add(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_add_const(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let variable_id = bytecode.read_u8()? as usize;
        let value = bytecode.read_u16::<BigEndian>()? as i16;
        self.variables[variable_id] += value;
        Ok(())
    }

    pub fn op_call(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let offset: u16 = bytecode.read_u16::<BigEndian>()?;
        debug!("op_call: offset {}", offset);

        self.stack.push(bytecode.position());
        bytecode.seek(SeekFrom::Start(offset as u64))?;
        Ok(())
    }

    pub fn op_ret(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let offset = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        context.loaded_part.bytecode.seek(SeekFrom::Start(offset))?;
        Ok(())
    }

    pub fn op_pause_thread(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        let current_channel = self.running_channel_id;
        self.channels[current_channel].state = State::Yielding;
        Ok(())
    }

    pub fn op_jmp(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let offset = bytecode.read_u16::<BigEndian>()?;
        bytecode.seek(SeekFrom::Start(offset as u64))?;
        Ok(())
    }

    pub fn op_set_vec(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let channel_id = bytecode.read_u8()?;
        let offset = bytecode.read_u16::<BigEndian>()?;
        self.channels[channel_id as usize].pending_setvec = Some(offset as usize);
        Ok(())
    }

    pub fn op_jnz(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let i = bytecode.read_u8()? as usize;
        self.variables[i] -= 1;
        if self.variables[i] != 0 {
            self.op_jmp(context)?;
        } else {
            bytecode.read_u16::<BigEndian>()?;
        }
        Ok(())
    }

    pub fn op_cond_jmp(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let opcode = bytecode.read_u8()?;
        let var = bytecode.read_u8()?;

        let a = match opcode {
            opcode if opcode & 0x80 != 0 => self.variables[bytecode.read_u8()? as usize],
            opcode if opcode & 0x40 != 0 => bytecode.read_u16::<BigEndian>()? as i16,
            _ => bytecode.read_u8()? as i16,
        };
        let b = self.variables[var as usize];

        let comparation = opcode & 7;
        let expr = match comparation {
            0 => a == b,
            1 => a != b,
            2 => b > a,
            3 => b >= a,
            4 => a > b,
            5 => a >= b,
            _ => false,
        };

        if expr {
            self.op_jmp(context)?;
        } else {
            bytecode.read_u16::<BigEndian>()?;
        }

        Ok(())
    }

    pub fn op_set_palette(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let palette_id = context.loaded_part.bytecode.read_u16::<BigEndian>()?;
        let palette_request = PaletteRequest::Change((palette_id >> 8) as u8);
        context.video.request_palette(palette_request);
        Ok(())
    }

    pub fn op_reset_threads(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_select_video_page(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let page_id = PageId::from(context.loaded_part.bytecode.read_u8()?);
        debug!("op_select_video_page: page_id: {:?}", page_id);

        context.video.change_working_buffer(page_id);
        Ok(())
    }

    pub fn op_fill_video_page(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let page_id = PageId::from(bytecode.read_u8()?);
        let color = bytecode.read_u8()?;
        debug!(
            "op_fill_video_page: page_id: {:?}, color: {:?}",
            page_id, color
        );
        context.video.fill_page(page_id, color);
        Ok(())
    }

    pub fn op_copy_video_page(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let src_page_id = PageId::from(bytecode.read_u8()?);
        let dst_page_id = PageId::from(bytecode.read_u8()?);
        context.video.copy_page(
            src_page_id,
            dst_page_id,
            self.variables[VM_VARIABLE_SCROLL_Y],
        );
        Ok(())
    }

    pub fn op_blit_frame_buffer(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let sleep = self.variables[VM_VARIABLE_PAUSE_SLICES] * 20;
        thread::sleep(Duration::from_millis(sleep as u64));
        self.variables[0xF7] = 0;

        let page_id = PageId::from(context.loaded_part.bytecode.read_u8()?);
        debug!("op_blit_frame_buffer: page_id {:?}", page_id);

        let video = &mut context.video;
        let palette = &mut context.loaded_part.palette;
        Ok(video.update_display(page_id, palette)?)
    }

    pub fn op_kill_thread(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        let current_channel = self.running_channel_id;
        self.channels[current_channel].state = State::Dead;
        Ok(())
    }

    pub fn op_draw_string(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_sub(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_and(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_or(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_shl(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_shr(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_play_sound(&mut self, _: &mut ExecutionContext) -> Result<(), VmError> {
        Ok(())
    }

    pub fn op_update_mem_list(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let resource_id = context.loaded_part.bytecode.read_u16::<BigEndian>()?;
        if resource_id == 0 {
            context.loaded_asset = LoadedAsset::default();
        } else {
            let asset = context.resource.load_entry(resource_id as usize)?;
            context
                .loaded_asset
                .assets
                .insert(resource_id as usize, asset);
        }
        Ok(())
    }

    pub fn op_play_music(&mut self, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        bytecode.read_u32::<BigEndian>()?;
        bytecode.read_u8()?;
        Ok(())
    }

    fn draw_sprite(&mut self, opcode: u8, context: &mut ExecutionContext) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let offset: u16 = bytecode.read_u16::<BigEndian>()? * 2;

        let mut x: i16 = bytecode.read_u8()? as i16;
        if opcode & 0x20 == 0 {
            if opcode & 0x10 == 0 {
                x = (x << 8) | bytecode.read_u8()? as i16;
            } else {
                x = self.variables[x as usize];
            }
        } else if opcode & 0x10 != 0 {
            x += 0x100;
        }

        let mut y: i16 = bytecode.read_u8()? as i16;
        if opcode & 8 == 0 {
            if opcode & 4 == 0 {
                y = (y << 8) | bytecode.read_u8()? as i16;
            } else {
                y = self.variables[y as usize];
            }
        }

        let mut zoom: u16 = bytecode.read_u8()? as u16;
        if opcode & 2 == 0 {
            if opcode & 1 == 0 {
                bytecode.seek(SeekFrom::Current(-1))?;
                zoom = 0x40;
            } else {
                zoom = self.variables[zoom as usize] as u16;
            }
        } else if opcode & 1 != 0 {
            bytecode.seek(SeekFrom::Current(-1))?;
            zoom = 0x40;
        }

        if opcode & 3 == 3 {
            let cinematic = &mut context.loaded_part.cinematic;
            cinematic.seek(SeekFrom::Start(offset as u64))?;
            context
                .video
                .read_and_draw_polygon(cinematic, 0xFF, zoom, Point { x, y })?
        }

        if let Some(ref mut polygon) = context.loaded_part.polygon {
            polygon.seek(SeekFrom::Start(offset as u64))?;
            Ok(context
                .video
                .read_and_draw_polygon(polygon, 0xFF, zoom, Point { x, y })?)
        } else {
            Err(VmError::MissingPolygonSegment)
        }
    }

    fn draw_background(
        &mut self,
        opcode: u8,
        context: &mut ExecutionContext,
    ) -> Result<(), VmError> {
        let bytecode = &mut context.loaded_part.bytecode;
        let offset = ((u16::from(opcode) << 8) | bytecode.read_u8()? as u16).wrapping_mul(2);
        let mut x: i16 = bytecode.read_u8()? as i16;
        let mut y: i16 = bytecode.read_u8()? as i16;
        let h: i16 = y - 199;
        if h > 0 {
            y = 199;
            x += h;
        }

        let color = 0xFF;
        let zoom = 0x40;
        let cinematic = &mut context.loaded_part.cinematic;
        cinematic.seek(SeekFrom::Start(offset as u64))?;
        Ok(context
            .video
            .read_and_draw_polygon(cinematic, color, zoom, Point { x, y })?)
    }
}
