mod app;
mod common;
mod blit_renderer;
mod mesh_renderer;
mod raytracer;
mod bvh;
mod scene;

use app::MainApp;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    pretty_env_logger::init();
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app_handler = common::AppHandler::<MainApp>::default();
    event_loop.run_app(&mut app_handler).expect("Failed to run app");
}