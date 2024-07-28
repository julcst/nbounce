use std::sync::Arc;
use std::time::Instant;

mod imgui_winit_support;

use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::{ElementState, KeyEvent, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, keyboard::{KeyCode, PhysicalKey}, window::{Window, WindowId}
};
use log::{error, info};

struct App<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Arc<Window>,
    imgui_renderer: imgui_wgpu::Renderer,
    imgui_ctx: imgui::Context,
    imgui_platform: imgui_winit_support::WinitPlatform,
    last_frame: Instant,
}

const TITLE: &str = "Pathtracer";

impl App<'_> {
    async fn new(window: Window) -> Self {
        let window_arc = Arc::new(window);

        let size = window_arc.inner_size().max(PhysicalSize::new(1, 1));

        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window_arc.clone()).expect("Failed to create surface");

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

        // Set up dear imgui
        let hidpi_factor = window_arc.scale_factor();
        let mut imgui_ctx = imgui::Context::create();
        let mut imgui_platform = imgui_winit_support::WinitPlatform::init(&mut imgui_ctx);
        imgui_platform.attach_window(
            imgui_ctx.io_mut(),
            &window_arc,
            imgui_winit_support::HiDpiMode::Default,
        );
        imgui_ctx.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        imgui_ctx.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        imgui_ctx.fonts().add_font(&[imgui::FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        // Set up dear imgui wgpu renderer
        let renderer_config = imgui_wgpu::RendererConfig {
            texture_format: config.format,
            ..Default::default()
        };

        let imgui_renderer = imgui_wgpu::Renderer::new(&mut imgui_ctx, &device, &queue, renderer_config);

        Self {
            window: window_arc,
            surface,
            device,
            queue,
            config,
            size,
            imgui_renderer,
            imgui_ctx,
            imgui_platform,
            last_frame: Instant::now(),
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame);
        self.last_frame = now;
        self.imgui_ctx.io_mut().update_delta_time(delta);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let ui = self.imgui_ctx.frame();

        {
            let window = ui.window("Hello world");
            window
                .size([300.0, 100.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello world!");
                    ui.text("This...is...imgui-rs on WGPU!");
                    ui.separator();
                    ui.button("Quit");
                });
        }

        self.imgui_platform.prepare_render(ui, &self.window);
        
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 20.0,
                            b: 30.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.imgui_renderer
                .render(self.imgui_ctx.render(), &self.queue, &self.device, &mut _render_pass)
                .expect("ImGui Rendering failed");
        }
    
        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[derive(Default)]
struct AppHandler<'a> {
    app: Option<App<'a>>,
}

impl ApplicationHandler for AppHandler<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop.create_window(Window::default_attributes().with_title(TITLE)).expect("Failed to create window");
        self.app = Some(pollster::block_on(App::new(window)));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let app = self.app.as_mut().unwrap();
        app.imgui_platform.handle_window_event(app.imgui_ctx.io_mut(), &app.window, &event);

        if window_id == app.window().id() && !app.input(&event) {
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
                        Err(wgpu::SurfaceError::Lost) => app.resize(app.size),
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
        app.imgui_platform
            .prepare_frame(app.imgui_ctx.io_mut(), &app.window)
            .expect("Failed to prepare frame");
        app.window().request_redraw();
    }
}

fn main() {
    pretty_env_logger::init();
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app_handler = AppHandler::default();
    let _ = event_loop.run_app(&mut app_handler);
}
