use std::{
    cmp::{max, min},
    io::{self, Cursor, Seek},
};

use byteorder::{BigEndian, ReadBytesExt};
use thiserror::Error;

use crate::{
    renderer::{Renderer, RendererError},
    shapes::{Point, Polygon},
};

const HEIGHT: usize = 200;
const WIDTH: usize = 320;
const VID_PAGE_SIZE: usize = HEIGHT * WIDTH / 2;

#[derive(Error, Debug)]
pub enum VideoError {
    #[error("Error in the underlying stream")]
    Io(io::Error),
    #[error("Renderer error")]
    RendererError(RendererError),
    #[error("Invalid palette number {0}")]
    InvalidPalette(u8),
    #[error("Unexpected command")]
    UnexpectedCommand,
}

impl From<io::Error> for VideoError {
    fn from(value: io::Error) -> Self {
        VideoError::Io(value)
    }
}

impl From<RendererError> for VideoError {
    fn from(value: RendererError) -> Self {
        VideoError::RendererError(value)
    }
}

#[derive(PartialEq, Debug)]
pub enum PageId {
    Numbered(u8),
    Front,
    Back,
}

impl PageId {
    pub fn from(raw_page_id: u8) -> PageId {
        match raw_page_id {
            0xFE => PageId::Front,
            0xFF => PageId::Back,
            n if n <= 3 => PageId::Numbered(n),
            _ => PageId::Numbered(0),
        }
    }
}

pub enum PaletteRequest {
    Change(u8),
    Keep,
}

pub struct Video {
    hline_y: i16,
    pages: [[u8; VID_PAGE_SIZE]; 4],
    work_buffer: usize,
    front_buffer: usize,
    back_buffer: usize,
    palette_req: PaletteRequest,
    renderer: Renderer,
}

impl Video {
    pub fn new(renderer: Renderer) -> Self {
        Video {
            hline_y: 0,
            pages: [[0; VID_PAGE_SIZE]; 4],
            work_buffer: 2,
            front_buffer: 2,
            back_buffer: 1,
            palette_req: PaletteRequest::Keep,
            renderer,
        }
    }

    fn draw_point(&mut self, x: i16, y: i16, color: u8) {
        if !(0..=319).contains(&x) || !(0..=199).contains(&y) {
            return;
        }
        let offset: usize = (y * 160 + x / 2) as usize;
        let (mut old_color_mask, mut new_color_mask): (u8, u8) = if x & 1 != 0 {
            (0xF0, 0x0F)
        } else {
            (0x0F, 0xF0)
        };
        let mut byte_color = color << 4 | color;
        if color == 0x10 {
            new_color_mask &= 0x88;
            old_color_mask = !new_color_mask;
            byte_color = 0x88;
        } else if color == 0x11 {
            byte_color = self.pages[0][offset];
        }
        let pixel_pair = self.pages[self.work_buffer][offset];
        self.pages[self.work_buffer][offset] =
            (pixel_pair & old_color_mask) | (byte_color & new_color_mask);
    }

    fn draw_line_normal(&mut self, x1: i16, x2: i16, color: u8) {
        let x_max = max(x1, x2);
        let x_min = min(x1, x2);
        let offset = (self.hline_y * 160 + x_min / 2) as usize;
        let width = (x_max / 2 - x_min / 2 + 1) as usize;
        let page = &mut self.pages[self.work_buffer];
        let byte_color = ((color & 0xF) << 4) | (color & 0xF);
        page[offset] = (page[offset] & 0xF0) | (byte_color & 0x0F);
        page[offset + width - 1] = (page[offset + width - 1] & 0x0F) | (byte_color & 0xF0);
        let start = (x_min & 1) as usize;
        let end = max(0, width as i16 - 1 - ((x_max & 1) ^ 1)) as usize;
        (start..=end).for_each(|i| {
            page[offset + i] = byte_color;
        });
    }

    fn draw_line_from_bg(&mut self, x1: i16, x2: i16) {
        let x_max = max(x1, x2);
        let x_min = min(x1, x2);
        let offset = (self.hline_y * 160 + x_min / 2) as usize;
        let width = (x_max / 2 - x_min / 2 + 1) as usize;
        let (left, right) = self.pages.split_at_mut(self.work_buffer);
        let bg_page = left[0];
        let work_page = &mut right[0];

        work_page[offset] = (work_page[offset] & 0xF0) | (bg_page[offset] & 0x0F);
        work_page[offset + width - 1] =
            (work_page[offset + width - 1] & 0x0F) | (bg_page[offset + width - 1] & 0xF0);
        let start = (x_min & 1) as usize;
        let end = max(0, width as i16 - 1 - ((x_max & 1) ^ 1)) as usize;
        (start..=end).for_each(|i| {
            work_page[offset + i] = bg_page[offset + i];
        });
    }

    fn draw_line_blend(&mut self, x1: i16, x2: i16) {
        let x_max = max(x1, x2);
        let x_min = min(x1, x2);
        let offset = (self.hline_y * 160 + x_min / 2) as usize;
        let width = (x_max / 2 - x_min / 2 + 1) as usize;
        let page = &mut self.pages[self.work_buffer];
        page[offset] |= 0x08;
        page[offset + width - 1] |= 0x80;
        let start = (x_min & 1) as usize;
        let end = max(0, width as i16 - 1 - ((x_max & 1) ^ 1)) as usize;
        (start..=end).for_each(|i| {
            page[offset + i] |= 0x88;
        });
    }

    pub fn change_working_buffer(&mut self, page_id: PageId) {
        self.work_buffer = self.get_page(page_id);
    }

    fn get_page(&mut self, page_id: PageId) -> usize {
        match page_id {
            PageId::Front => self.front_buffer,
            PageId::Back => self.back_buffer,
            PageId::Numbered(n) if n <= 3 => n as usize,
            _ => panic!("Unsupported PageId pattern"),
        }
    }

    fn read_and_draw_polygon_hierarchy(
        &mut self,
        stream: &mut Cursor<Vec<u8>>,
        zoom: u16,
        pgc: Point,
    ) -> Result<(), VideoError> {
        let pt = Point {
            x: pgc.x - (stream.read_u8()? as i32 * zoom as i32 / 64) as i16,
            y: pgc.y - (stream.read_u8()? as i32 * zoom as i32 / 64) as i16,
        };
        let childs = stream.read_u8()?;
        for _ in 0..=childs {
            let mut offset = stream.read_u16::<BigEndian>()?;
            let po = Point {
                x: pt.x + (stream.read_u8()? as i32 * zoom as i32 / 64) as i16,
                y: pt.y + (stream.read_u8()? as i32 * zoom as i32 / 64) as i16,
            };
            let mut color = 0xFF;
            let bp = offset;
            offset &= 0x7FFF;

            if bp & 0x8000 != 0 {
                color = stream.read_u8()? & 0x7F;
                stream.seek(io::SeekFrom::Current(1))?;
            }
            let bkp_offset = stream.position();
            stream.set_position((offset * 2) as u64);
            self.read_and_draw_polygon(stream, color, zoom, po)?;

            stream.set_position(bkp_offset);
        }
        Ok(())
    }

    pub fn read_and_draw_polygon(
        &mut self,
        stream: &mut Cursor<Vec<u8>>,
        mut color: u8,
        zoom: u16,
        pt: Point,
    ) -> Result<(), VideoError> {
        let command = stream.read_u8()?;
        if command >= 0xC0 {
            if (color & 0x80) != 0 {
                color = command & 0x3F;
            }
            let polygon = Polygon::read_vertices(stream, zoom)?;
            self.fill_polygon(color, pt, polygon);
        } else {
            let sub_command = command & 0x3F;
            match sub_command {
                2 => self.read_and_draw_polygon_hierarchy(stream, zoom, pt)?,
                _ => return Err(VideoError::UnexpectedCommand),
            }
        }
        Ok(())
    }

    fn fill_polygon(&mut self, color: u8, pt: Point, polygon: Polygon) {
        if polygon.bbw == 0 && polygon.bbh == 1 && polygon.points.len() == 4 {
            self.draw_point(pt.x, pt.y, color);
        }

        let x1 = pt.x - polygon.bbw / 2;
        let x2 = pt.x + polygon.bbw / 2;
        let y1 = pt.y - polygon.bbh / 2;
        let y2 = pt.y + polygon.bbh / 2;

        if x1 > 319 || x2 < 0 || y1 > 199 || y2 < 0 {
            return;
        }

        self.hline_y = y1;
        for i in 0..polygon.points.len() / 2 {
            let curr_left_p = &polygon.points[polygon.points.len() - 1 - i];
            let next_left_p = &polygon.points[polygon.points.len() - 2 - i];
            let curr_right_p = &polygon.points[i];
            let next_right_p = &polygon.points[i + 1];

            let step_left = self.calc_step(curr_left_p, next_left_p);
            let step_right = self.calc_step(curr_right_p, next_right_p);
            let h_diff = next_left_p.y - curr_left_p.y;

            if h_diff > 0 {
                let mut x_left = curr_left_p.x as f64 + x1 as f64;
                let mut x_right = curr_right_p.x as f64 + x1 as f64;
                for _ in 0..h_diff {
                    if self.hline_y >= 0 && x_left <= 319.0 && x_right >= 0.0 {
                        let mut draw_left = x_left.round() as i16;
                        let mut draw_right = x_right.round() as i16;
                        draw_left = max(0, draw_left);
                        draw_right = min(draw_right, 319);
                        match color {
                            c if c < 0x10 => self.draw_line_normal(draw_left, draw_right, color),
                            c if c > 0x10 => self.draw_line_from_bg(draw_left, draw_right),
                            _ => self.draw_line_blend(draw_left, draw_right),
                        }
                    }
                    x_left += step_left;
                    x_right += step_right;
                    self.hline_y += 1;
                    if self.hline_y > 199 {
                        return;
                    }
                }
            }
        }
    }

    pub fn fill_page(&mut self, page_id: PageId, color: u8) {
        let page = &mut self.pages[self.get_page(page_id)];
        let byte_color = (color << 4) | color;
        page.fill(byte_color);
    }

    pub fn copy_page(&mut self, mut src_page_id: PageId, dst_page_id: PageId, vscroll: i16) {
        if src_page_id == dst_page_id {
            return;
        }

        let is_vertical_scrolled = matches!(src_page_id, PageId::Numbered(n) if n & 80 != 0);
        let src_mask = if is_vertical_scrolled { 3 } else { 0xBF };
        if let PageId::Numbered(n) = src_page_id {
            src_page_id = PageId::Numbered(n & src_mask)
        };

        let (raw_src_page_id, raw_dst_page_id) =
            (self.get_page(src_page_id), self.get_page(dst_page_id));

        let (src_page, dst_page) = if raw_src_page_id < raw_dst_page_id {
            let (l, r) = self.pages.split_at_mut(raw_dst_page_id);
            (&l[raw_src_page_id], &mut r[0])
        } else {
            let (l, r) = self.pages.split_at_mut(raw_src_page_id);
            (&r[0], &mut l[raw_dst_page_id])
        };

        if is_vertical_scrolled && vscroll.abs() <= 199 {
            let data_to_copy = HEIGHT - vscroll.unsigned_abs() as usize;
            let (src_offset, dst_offset) = if vscroll < 0 {
                (-vscroll as usize, 0)
            } else {
                (0, vscroll as usize)
            };
            dst_page[dst_offset..dst_offset + data_to_copy]
                .copy_from_slice(&src_page[src_offset..src_offset + data_to_copy]);
        } else {
            dst_page.copy_from_slice(src_page);
        }
    }

    pub fn copy_bg(&mut self, src_data: &[u8]) {
        let mut bg_page = self.pages[0];
        for h in 0..HEIGHT {
            let bytes_per_row = WIDTH / 8;
            for w in 0..bytes_per_row {
                let plane_offset = HEIGHT * WIDTH / 8;
                let mut planar_palette_idx = [
                    (*src_data)[h * bytes_per_row + w + plane_offset * 3],
                    (*src_data)[h * bytes_per_row + w + plane_offset * 2],
                    (*src_data)[h * bytes_per_row + w + plane_offset],
                    (*src_data)[h * bytes_per_row + w],
                ];
                for byte in 0..4 {
                    let mut acc: u8 = 0;
                    for bit in 0..8 {
                        acc <<= 1;
                        acc |= ((planar_palette_idx[bit & 3] & 0x80) != 0) as u8;
                        planar_palette_idx[bit & 3] <<= 1;
                    }
                    bg_page[h * bytes_per_row + w + byte] = acc;
                }
            }
        }
    }

    fn calc_step(&self, p1: &Point, p2: &Point) -> f64 {
        let dy = p2.y - p1.y;
        let dx = p2.x - p1.x;
        dx as f64 / dy as f64
    }

    pub fn request_palette(&mut self, palette_request: PaletteRequest) {
        self.palette_req = palette_request;
    }

    fn change_palette(
        &mut self,
        palette_id: u8,
        palette_segment: &mut Cursor<Vec<u8>>,
    ) -> Result<(), VideoError> {
        if palette_id >= 32 {
            return Err(VideoError::InvalidPalette(palette_id));
        }
        palette_segment.seek(io::SeekFrom::Start(palette_id as u64 * 32))?;
        Ok(self.renderer.set_palette(palette_segment)?)
    }

    pub fn update_display(
        &mut self,
        page_id: PageId,
        palette_segment: &mut Cursor<Vec<u8>>,
    ) -> Result<(), VideoError> {
        if matches!(page_id, PageId::Numbered(_)) {
            self.front_buffer = self.get_page(page_id);
        } else if matches!(page_id, PageId::Back) {
            (self.front_buffer, self.back_buffer) = (self.back_buffer, self.front_buffer);
        }

        if let PaletteRequest::Change(palette_id) = self.palette_req {
            self.change_palette(palette_id, palette_segment)?;
            self.palette_req = PaletteRequest::Keep;
        }

        Ok(self
            .renderer
            .update_display(&self.pages[self.front_buffer])?)
    }
}
