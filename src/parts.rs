use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::EnumCount;

#[derive(IntoPrimitive, TryFromPrimitive, PartialEq, Eq, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Segment {
    Palette,
    Bytecode,
    PolyCinematic,
    Polygon,
}

#[derive(Copy, Clone, IntoPrimitive, TryFromPrimitive, EnumCount)]
#[repr(u16)]
pub enum GamePart {
    One = 0x3E80,
    Two = 0x3E81,
    Tree = 0x3E82,
    Four = 0x3E83,
    Five = 0x3E84,
    Six = 0x3E85,
    Seven = 0x3E86,
    Eigth = 0x3E87,
    Nine = 0x3E88,
    Ten = 0x3E89,
}

const NUM_PARTS: usize = GamePart::COUNT;
pub static SEGMENT_IDX_BY_PART: [[usize; 4]; NUM_PARTS] = [
    [0x14, 0x15, 0x16, 0x00],
    [0x17, 0x18, 0x19, 0x00],
    [0x1A, 0x1B, 0x1C, 0x11],
    [0x1D, 0x1E, 0x1F, 0x11],
    [0x20, 0x21, 0x22, 0x11],
    [0x23, 0x24, 0x25, 0x00],
    [0x26, 0x27, 0x28, 0x11],
    [0x29, 0x2A, 0x2B, 0x11],
    [0x7D, 0x7E, 0x7F, 0x00],
    [0x7D, 0x7E, 0x7F, 0x00],
];
