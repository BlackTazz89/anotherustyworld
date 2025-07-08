use std::{process, time::Duration};

use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    platform::pump_events::EventLoopExtPumpEvents,
};

pub struct SysEventHandler {
    event_loop: EventLoop<()>,
}

impl SysEventHandler {
    pub fn new(event_loop: EventLoop<()>) -> Self {
        Self { event_loop }
    }

    pub fn pump_events(&mut self) {
        self.event_loop
            .pump_events(Some(Duration::ZERO), |event, _| {
                if let Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } = event
                {
                    process::exit(0);
                }
            });
    }
}
