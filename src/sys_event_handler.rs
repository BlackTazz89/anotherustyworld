use winit::event_loop::EventLoop;

pub struct SysEventHandler {
    event_loop: EventLoop<()>,
}

impl SysEventHandler {
    pub fn new(event_loop: EventLoop<()>) -> Self {
        Self { event_loop }
    }
}
