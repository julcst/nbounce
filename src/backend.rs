
use std::sync::Arc;

use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::{ElementState, KeyEvent, WindowEvent}, event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey}, window::{Window, WindowId}
};
use log::{error, info};

use crate::imgui_winit_support;

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
        let app = self.app.as_mut().unwrap();

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
                    app.resize(new_size)
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
                        Err(e) => error!("{:?}", e),
                    }
                }
                _ => (),
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let app = self.app.as_mut().unwrap();
        app.window().request_redraw();
    }
}

pub struct WGPUContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

impl WGPUContext {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::default();

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor::default(),
                None
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        info!("Surface capabilities: {:?}", surface_caps);

        let size = window.inner_size().max(PhysicalSize::new(1, 1));

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb, // TODO: Use Rgba16Float, but it's not supported with imgui-wgpu
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

pub struct ImGuiContext {
    pub renderer: imgui_wgpu::Renderer,
    pub ctx: imgui::Context,
    pub platform: imgui_winit_support::WinitPlatform,
}

impl ImGuiContext {
    pub fn new(window: Arc<Window>, wgpu: &WGPUContext) -> Self {
        let hidpi_factor = window.scale_factor();
        let mut ctx = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut ctx);
        platform.attach_window(
            ctx.io_mut(),
            &window,
            imgui_winit_support::HiDpiMode::Default,
        );
        ctx.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        ctx.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        ctx.fonts().add_font(&[imgui::FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        // Set up dear imgui wgpu renderer
        let renderer_config = imgui_wgpu::RendererConfig {
            texture_format: wgpu.config.format,
            ..Default::default()
        };

        let renderer = imgui_wgpu::Renderer::new(&mut ctx, &wgpu.device, &wgpu.queue, renderer_config);

        Self {
            renderer,
            ctx,
            platform,
        }
    }

    pub fn update(&mut self, delta: std::time::Duration) {
        self.ctx.io_mut().update_delta_time(delta);
    }

    pub fn frame(&mut self, window: &Window) -> &mut imgui::Ui {
        self.platform
            .prepare_frame(self.ctx.io_mut(), &window)
            .expect("Failed to prepare ImGui frame");
        self.ctx.frame()
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        self.platform.handle_window_event(self.ctx.io_mut(), &window, &event);
    }

    pub fn prepare_render(&mut self, ui: &imgui::Ui, window: &Window) {
        self.platform.prepare_render(ui, &window)
    }

    pub fn render<'r>(&'r mut self, wgpu: &WGPUContext, render_pass: &mut wgpu::RenderPass<'r>) {
        self.renderer
            .render(self.ctx.render(), &wgpu.queue, &wgpu.device, render_pass)
            .expect("ImGui Rendering failed")
    }
}