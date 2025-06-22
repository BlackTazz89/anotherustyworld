use std::io::{self, Cursor};

use byteorder::ReadBytesExt;

const MAX_POINTS: usize = 64;

#[derive(Debug, Clone)]
pub struct Point {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug)]
pub struct Polygon {
    pub bbw: i16,
    pub bbh: i16,
    pub points: Vec<Point>,
}

impl Polygon {
    pub fn read_vertices(stream: &mut Cursor<Vec<u8>>, zoom: u16) -> Result<Polygon, io::Error> {
        let bbw = stream.read_u8()? as i16 * zoom as i16 / 64;
        let bbh = stream.read_u8()? as i16 * zoom as i16 / 64;
        let num_points = stream.read_u8()? as usize;

        assert!(num_points % 2 == 0, "Points must be even");
        assert!(num_points < MAX_POINTS, "Points must be max {MAX_POINTS}");

        let mut points: Vec<Point> = Vec::with_capacity(num_points);
        for _ in 0..num_points {
            let x = stream.read_u8()? as i16 * zoom as i16 / 64;
            let y = stream.read_u8()? as i16 * zoom as i16 / 64;
            points.push(Point { x, y });
        }
        Ok(Polygon { bbw, bbh, points })
    }
}
