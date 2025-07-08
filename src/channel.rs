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
    Paused,
    Dead,
}

#[derive(Copy, Clone)]
pub struct Channel {
    pub state: State,
    pub pc: ProcessCounter,
    pub next_pc: Option<ProcessCounter>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            state: State::Dead,
            pc: ProcessCounter::Invalid,
            next_pc: None,
        }
    }
}

impl Channel {
    pub fn reset(&mut self) {
        self.state = State::Dead;
        self.pc = ProcessCounter::Invalid;
        self.next_pc = None;
    }

    pub fn set_pc(&mut self, pc: ProcessCounter) {
        self.pc = pc;
        self.state = match pc {
            ProcessCounter::Valid(_) => State::Ready,
            ProcessCounter::Invalid => State::Dead,
        }
    }

    pub fn apply_next_pc(&mut self) {
        if let Some(next_pc) = self.next_pc {
            self.set_pc(next_pc);
            self.next_pc = None;
        };
    }

    pub fn yield_control(&mut self, execution_pc: ProcessCounter) {
        self.set_pc(execution_pc);
    }
}
