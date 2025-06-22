use std::{
    io::{self, Cursor},
    num::NonZeroU32,
};

use byteorder::{BigEndian, ReadBytesExt};
use softbuffer::{Context, SoftBufferError, Surface};
use thiserror::Error;
use winit::window::Window;

const SCALE_FACTOR: usize = 3;
const SCREEN_W: usize = 320;
const SCREEN_H: usize = 200;
pub const SCALED_H: usize = SCREEN_H * SCALE_FACTOR;
pub const SCALED_W: usize = SCREEN_W * SCALE_FACTOR;
const NUM_COLORS: usize = 16;

#[derive(Error, Debug)]
pub enum RendererError {
    #[error("Error in the underlying stream")]
    Io(io::Error),
    #[error("Error during softbuffer creation")]
    Softbuffer(SoftBufferError),
    #[error("Impossible resize surface")]
    SurfaceResize,
}

impl From<io::Error> for RendererError {
    fn from(value: io::Error) -> Self {
        RendererError::Io(value)
    }
}

impl From<SoftBufferError> for RendererError {
    fn from(value: SoftBufferError) -> Self {
        RendererError::Softbuffer(value)
    }
}

pub struct Renderer {
    window: Window,
    palette: [u32; NUM_COLORS],
}

impl Renderer {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            palette: Default::default(),
        }
    }

    pub fn set_palette(&mut self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), RendererError> {
        for i in 0..NUM_COLORS {
            let color444 = cursor.read_u16::<BigEndian>()?;
            let mut r = (color444 & 0x0F00) >> 8;
            let mut g = (color444 & 0xF0) >> 4;
            let mut b = color444 & 0x0F;
            r |= r << 4;
            g |= g << 4;
            b |= b << 4;
            self.palette[i] = (u32::from(r) << 16) | (u32::from(g) << 8) | b as u32;
        }
        Ok(())
    }

    pub fn update_display(&mut self, src: &[u8]) -> Result<(), RendererError> {
        let context = Context::new(&self.window).unwrap();

        let mut surface = Surface::new(&context, &self.window).unwrap();
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Err(RendererError::SurfaceResize);
        };
        surface.resize(width, height)?;

        let mut dest = surface.buffer_mut()?;
        let src_lines = src.chunks_exact(SCREEN_W / 2);
        let dest_lines = dest.chunks_exact_mut(SCALED_W * SCALE_FACTOR);

        for (src_line, dest_line) in src_lines.zip(dest_lines) {
            for (i, &two_pixels_byte) in src_line.iter().enumerate() {
                let left_pixel_index = (two_pixels_byte >> 4) as usize;
                let right_pixel_index = (two_pixels_byte & 0x0F) as usize;

                let left_color = self.palette[left_pixel_index];
                let right_color = self.palette[right_pixel_index];

                for y in 0..SCALE_FACTOR {
                    for x in 0..SCALE_FACTOR {
                        let current_row_idx = SCALED_W * y;
                        let curr_col_idx = i * 2 * SCALE_FACTOR;
                        dest_line[current_row_idx + curr_col_idx + x] = left_color;
                        dest_line[current_row_idx + curr_col_idx + SCALE_FACTOR + x] = right_color;
                    }
                }
            }
        }
        dest.present()?;
        Ok(())
    }
}
