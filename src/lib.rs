mod main_window_graphics;
mod minesweeper;
mod window;

use pollster::FutureExt;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop,
    keyboard::{
        KeyCode,
        PhysicalKey,
    },
    window::{
        Window,
        WindowAttributes,
        WindowId,
    },
};

const DEFAULT_WIDTH: minesweeper::Dim = 10;
const DEFAULT_HEIGHT: minesweeper::Dim = 10;
const DEFAULT_MINES: minesweeper::Count = 20;

/// The State of a  Minesweeper game process.
struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    main_window_graphics: main_window_graphics::MainWindowGraphics,
    minesweeper_game: minesweeper::Game,
    cursor_pos: cgmath::Vector2<f32>,
    game_start_time: std::time::Instant,
    // The window must be declared after the surface so
    // it gets dropped after it as the surface contains
    // unsafe references to the window's resources.
    window: Arc<Window>,
}

impl<'a> State<'a> {
    /// Creates a new State.
    /// It is async as creating some of the wgpu types requires async code.
    fn new(window: Arc<Window>, minesweeper_game: minesweeper::Game) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        // Handle for the window
        let surface = instance.create_surface(window.clone()).unwrap();

        // Adapter for instance
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .block_on()
            .unwrap();

        // Adapter provides device for allocating GPU memory and queue editing GPU memory
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web, we'll have to disable some.
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .block_on()
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        // Shader code uses sRGB surface textures
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Set up textures for grid
        let main_window_graphics = main_window_graphics::MainWindowGraphics::new(
            &device,
            &queue,
            config.format,
            &minesweeper_game,
        );

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            main_window_graphics,
            cursor_pos: cgmath::Vector2::new(0.0, 0.0),
            minesweeper_game,
            game_start_time: std::time::Instant::now(),
        }
    }

    /// Handles updating the State with a new window size.
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.main_window_graphics.rescale(&self.size);
        }
    }

    /// Handles user inputs to the window.
    fn input(&mut self, event: &WindowEvent, event_loop: &event_loop::ActiveEventLoop) -> bool {
        let flags = self.minesweeper_game.flags;
        let game_state = self.minesweeper_game.game_state.clone();
        let result = match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyR),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                self.minesweeper_game.reset();
                self.main_window_graphics.reset_grid();
                self.window.request_redraw();
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scaling_x = self.main_window_graphics.scaling_x();
                let scaling_y = self.main_window_graphics.scaling_y();
                self.cursor_pos.x =
                    (position.x as f32 / self.size.width as f32 - 0.5) / scaling_x * 2.0;
                self.cursor_pos.y =
                    (position.y as f32 / self.size.height as f32 - 0.5) / scaling_y * -2.0;
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                let grid_pos = main_window_graphics::convert_to_over_grid(
                    self.minesweeper_game.width,
                    self.minesweeper_game.height,
                    self.cursor_pos,
                );
                if let Some(pos) = grid_pos {
                    let result = if button == &MouseButton::Left {
                        self.minesweeper_game.left_click(pos)
                    } else if button == &MouseButton::Right {
                        self.minesweeper_game.right_click(pos)
                    } else {
                        Vec::new()
                    };
                    self.main_window_graphics.update_grid(result);
                    self.window.request_redraw();
                }
                if main_window_graphics::is_face_pressed(
                    self.minesweeper_game.width,
                    self.minesweeper_game.height,
                    self.cursor_pos,
                ) {
                    if button == &MouseButton::Left {
                        self.minesweeper_game.reset();
                        self.main_window_graphics.reset_grid();
                        self.window.request_redraw();
                    }
                }
                true
            }
            _ => false,
        };
        // If the grid changed, check if displays need to be updated
        if result {
            if flags != self.minesweeper_game.flags {
                let val =
                    self.minesweeper_game.total_mines as i32 - self.minesweeper_game.flags as i32;
                self.main_window_graphics
                    .update_display(main_window_graphics::Display::MinesUnflagged, val);
            }
            if game_state != self.minesweeper_game.game_state {
                use minesweeper::GameState::*;
                event_loop.set_control_flow(match self.minesweeper_game.game_state {
                    BeforeGame => {
                        self.main_window_graphics
                            .update_display(main_window_graphics::Display::Timer, 0);
                        self.main_window_graphics.update_display(
                            main_window_graphics::Display::MinesUnflagged,
                            self.minesweeper_game.total_mines as i32,
                        );
                        self.window.request_redraw();
                        event_loop::ControlFlow::Wait
                    }
                    DuringGame => {
                        self.game_start_time = std::time::Instant::now();
                        event_loop::ControlFlow::WaitUntil(
                            self.game_start_time + std::time::Duration::from_secs_f32(1.0),
                        )
                    }
                    AfterGame => {
                        let game_duration_ms = self.game_start_time.elapsed().as_millis();
                        let game_duration_seconds = game_duration_ms / 1000;
                        println!(
                            "Game duration: {}.{} seconds",
                            game_duration_seconds,
                            game_duration_ms % 1000
                        );
                        event_loop::ControlFlow::Wait
                    }
                });
            }
        };
        result
    }

    /// Render the game to the window.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.main_window_graphics
                .render(&mut render_pass, &self.device, &self.queue);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

enum MinesweeperApp<'a> {
    Suspended(Option<minesweeper::Game>),
    Running(State<'a>),
}

impl<'a> ApplicationHandler for MinesweeperApp<'a> {
    fn new_events(&mut self, event_loop: &event_loop::ActiveEventLoop, cause: StartCause) {
        let state = match self {
            MinesweeperApp::Suspended(..) => return,
            MinesweeperApp::Running(state) => state,
        };

        match cause {
            StartCause::Init => (),
            StartCause::ResumeTimeReached {
                start: _,
                requested_resume,
            } => {
                event_loop.set_control_flow(event_loop::ControlFlow::WaitUntil(
                    requested_resume + std::time::Duration::from_secs_f32(1.0),
                ));
                state.main_window_graphics.update_display(
                    main_window_graphics::Display::Timer,
                    state.game_start_time.elapsed().as_secs() as i32,
                );
                state.window.request_redraw();
            }
            StartCause::WaitCancelled { .. } => (),
            StartCause::Poll => panic!(),
        }
    }

    fn resumed(&mut self, event_loop: &event_loop::ActiveEventLoop) {
        match self {
            MinesweeperApp::Running(..) => panic!("Minesweeper handler already running"),
            MinesweeperApp::Suspended(game) => {
                let game =
                    std::mem::replace(game, None).expect("App suspended without storing game");
                let window = Arc::new(
                    event_loop
                        .create_window(WindowAttributes::default())
                        .unwrap(),
                );
                window.set_title("Minesweeper");
                std::mem::swap(
                    self,
                    &mut MinesweeperApp::Running(State::new(window, game)),
                );
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self {
            MinesweeperApp::Suspended(..) => return,
            MinesweeperApp::Running(state) => state,
        };

        if window_id == state.window.id() {
            if !state.input(&event, event_loop) {
                match event {
                    WindowEvent::RedrawRequested => match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            state.resize(state.size);
                        }
                        Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("Out of memory");
                            event_loop.exit();
                        }
                        Err(wgpu::SurfaceError::Other) => {
                            log::error!("Other error (God knows)");
                            event_loop.exit();
                        }
                    },
                    WindowEvent::Resized(physical_size) => {
                        state.resize(physical_size);
                        state.window.request_redraw();
                    }
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                ..
                            },
                        ..
                    } => event_loop.exit(),
                    _ => {}
                }
            }
        }
    }

    fn suspended(&mut self, event_loop: &event_loop::ActiveEventLoop) {
        let state = std::mem::replace(self, MinesweeperApp::Suspended(None));
        if let MinesweeperApp::Running(state) = state {
            if let MinesweeperApp::Suspended(game) = self {
                std::mem::swap(game, &mut Some(state.minesweeper_game));
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &event_loop::ActiveEventLoop) {}

    fn device_event(
        &mut self,
        event_loop: &event_loop::ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
    }

    fn exiting(&mut self, event_loop: &event_loop::ActiveEventLoop) {}

    fn memory_warning(&mut self, event_loop: &event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &event_loop::ActiveEventLoop, event: ()) {}
}

/// Sets up the window and state and runs the event loop.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    env_logger::init();
    let event_loop = event_loop::EventLoop::new().unwrap();
    event_loop
        .run_app(&mut MinesweeperApp::Suspended(Some(
            minesweeper::Game::new(DEFAULT_WIDTH, DEFAULT_HEIGHT, DEFAULT_MINES),
        )))
        .expect("Event loop crashed!");
}
