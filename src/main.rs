use app::MainApp;
use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod backend;
mod imgui_winit_support;

fn main() {
    pretty_env_logger::init();
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app_handler = backend::AppHandler::<MainApp>::default();
    let _ = event_loop.run_app(&mut app_handler);
}
