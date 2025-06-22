#[derive(Copy, Clone, Debug)]
pub enum ProcessCounter {
    Valid(usize),
    Invalid,
}

impl From<u64> for ProcessCounter {
    fn from(value: u64) -> Self {
        if value >= 0xFFFE {
            return ProcessCounter::Invalid;
        }
        ProcessCounter::Valid(value as usize)
    }
}

#[derive(PartialEq, Copy, Clone)]
pub enum State {
    Ready,
    Running,
    Yielding,
    Dead,
}

#[derive(Copy, Clone)]
pub struct Channel {
    pub state: State,
    pub pc: ProcessCounter,
    pub pending_setvec: Option<usize>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            state: State::Ready,
            pc: ProcessCounter::Invalid,
            pending_setvec: None,
        }
    }
}

impl Channel {
    pub fn reset(&mut self) {
        self.state = State::Ready;
        self.pc = ProcessCounter::Invalid;
        self.pending_setvec = None;
    }
}
