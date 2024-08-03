use std::sync::Arc;

use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{ElementState, KeyEvent, WindowEvent}, event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey}, window::{Window, WindowId}};

pub trait App {
    async fn new(window: Arc<Window>) -> Self;
    fn window(&self) -> &Window;
    fn resize(&mut self, new_size: PhysicalSize<u32>);
    fn handle_input(&mut self, event: &WindowEvent);
    fn update(&mut self);
    fn render(&mut self) -> Result<(), wgpu::SurfaceError>;
}

pub struct AppHandler<T: App> {
    app: Option<T>,
}

impl<T: App> Default for AppHandler<T> {
    fn default() -> Self {
        Self { app: None }
    }
}

impl<T: App> ApplicationHandler for AppHandler<T> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(Window::default_attributes()).expect("Failed to create window"));
        self.app = Some(pollster::block_on(T::new(window)));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if let Some(app) = self.app.as_mut() {
            if window_id == app.window().id() {
                app.handle_input(&event);
                match event {
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                ..
                            },
                        ..
                    } => {
                        event_loop.exit()
                    },
                    WindowEvent::Resized(new_size) => {
                        app.resize(new_size);
                        app.window().request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        app.update();
                        match app.render() {
                            Ok(_) => {}
                            // Reconfigure the surface if lost
                            Err(wgpu::SurfaceError::Lost) => app.resize(app.window().inner_size()),
                            // The system is out of memory, we should probably quit
                            Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                            // All other errors (Outdated, Timeout) should be resolved by the next frame
                            Err(e) => log::error!("{:?}", e),
                        }
                        app.window().request_redraw();
                    }
                    _ => (),
                }
            }
        }
    }
}