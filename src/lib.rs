mod main_window_graphics;
mod minesweeper;
mod starting_params;

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

/// The State of a  Minesweeper game process.
struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    main_window_graphics: main_window_graphics::MainWindowGraphics,
    game: minesweeper::Game,
    cursor_pos: cgmath::Vector2<f32>,
    left_mouse_down: bool,
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
            left_mouse_down: false,
            game: minesweeper_game,
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

    /// Updates the [Display] in the main window based on self's internal data.
    fn update_display(&mut self, display: main_window_graphics::Display) {
        use main_window_graphics::Display;
        let new_val = match display {
            Display::Timer => self.game_start_time.elapsed().as_secs() as i32,
            Display::MinesUnflagged => self.game.total_mines as i32 - self.game.flags as i32,
        };
        self.main_window_graphics.update_display(display, new_val);
    }

    /// Updates the face textures if needed based on the change in mouse position, left_mouse_down,
    /// and game_state.
    /// Returns whether an update was made.
    fn update_face(
        &mut self,
        old_pos: cgmath::Vector2<f32>,
        old_left_mouse_down: bool,
        old_game_state: minesweeper::GameState,
    ) -> bool {
        let is_face_pressed = |pos, left_mouse_down| {
            left_mouse_down
                && main_window_graphics::is_over_face(self.game.width, self.game.height, pos)
        };
        let get_face = |pos, left_mouse_down, game_state| {
            main_window_graphics::face_from_game_state(
                is_face_pressed(pos, left_mouse_down),
                left_mouse_down,
                game_state,
            )
        };

        let old_face = get_face(old_pos, old_left_mouse_down, &old_game_state);
        let new_face = get_face(self.cursor_pos, self.left_mouse_down, &self.game.game_state);

        if old_face == new_face {
            return false;
        }

        self.main_window_graphics.update_face(new_face);
        true
    }

    /// Updates the grid textures if needed based on the change in mouse position, left_mouse_down,
    /// and game_state.
    /// Returns whether an update was made.
    /// Will "unpress" the old grid position and "press" the new position.
    fn update_grid(
        &mut self,
        old_pos: cgmath::Vector2<f32>,
        old_left_mouse_down: bool,
        old_game_state: minesweeper::GameState,
    ) -> bool {
        // Gets an Optional grid position from a mouse position
        // Is none if the mouse isn't over the grid or mouse isn't down
        let get_grid_pos =
            |cursor_pos, left_mouse_down: bool, game_state: minesweeper::GameState| {
                if left_mouse_down && !game_state.is_after_game() {
                    main_window_graphics::convert_to_over_grid(
                        self.game.width,
                        self.game.height,
                        cursor_pos,
                    )
                } else {
                    None
                }
            };

        // Get grid positions old the new cursor positions
        let old_grid_pos = get_grid_pos(old_pos, old_left_mouse_down, old_game_state);
        let new_grid_pos =
            get_grid_pos(self.cursor_pos, self.left_mouse_down, self.game.game_state);

        // Update window if cursor has moved cells
        if old_grid_pos != new_grid_pos {
            // Updates the cell at the given position to the given image if it is hidden
            // Returns whether an update was made
            let mut update_grid_pos = |pos, image| {
                if self.game.get_image_at(pos) == minesweeper::CellImage::Hidden {
                    // Update grid if cell is hidden
                    self.main_window_graphics.update_grid(&[(pos, image)]);
                    true
                } else {
                    false
                }
            };

            // Tries to hide the cell at the old position and "press" the cell at new position
            // Using map since the positions are options (i.e. cursor might move from grid to off
            // grid)
            let updated_old_pos = old_grid_pos
                .map(|pos| update_grid_pos(pos, self.game.get_image_at(pos)))
                .unwrap_or(false);
            let updated_new_pos = new_grid_pos
                .map(|pos| update_grid_pos(pos, minesweeper::CellImage::Zero))
                .unwrap_or(false);

            // Return whether changes were made
            updated_old_pos || updated_new_pos
        } else {
            false
        }
    }

    /// Updates the grid and face textures if needed based on the change in mouse position,
    /// left_mouse_down, and game_state.
    /// Returns whether an update was made.
    fn update_grid_and_face(
        &mut self,
        new_pos: cgmath::Vector2<f32>,
        new_left_mouse_down: bool,
        new_game_state: minesweeper::GameState,
    ) -> bool {
        let grid_updated = self.update_grid(new_pos, new_left_mouse_down, new_game_state);
        let face_updated = self.update_face(new_pos, new_left_mouse_down, new_game_state);

        grid_updated || face_updated
    }

    /// Updates the position of the cursor and updates the window if needed.
    fn move_cursor(&mut self, new_pos: &winit::dpi::PhysicalPosition<f64>) {
        // Calculate new position
        let scaling_x = self.main_window_graphics.scaling_x();
        let scaling_y = self.main_window_graphics.scaling_y();
        let new_pos_x = (new_pos.x as f32 / self.size.width as f32 - 0.5) / scaling_x * 2.0;
        let new_pos_y = (new_pos.y as f32 / self.size.height as f32 - 0.5) / scaling_y * -2.0;
        let new_pos = cgmath::vec2(new_pos_x, new_pos_y);

        // Update position in self but keep old position
        let old_pos = self.cursor_pos;
        self.cursor_pos = new_pos;

        // Try update grid and face
        let redraw_requested =
            self.update_grid_and_face(old_pos, self.left_mouse_down, self.game.game_state);

        // Request redraw if needed
        if redraw_requested {
            self.window.request_redraw();
        }
    }

    /// Sets left_mouse_down to true and updates the window if needed.
    fn left_mouse_down(&mut self) {
        self.left_mouse_down = true;
        let redraw_requested =
            self.update_grid_and_face(self.cursor_pos, false, self.game.game_state);
        if redraw_requested {
            self.window.request_redraw();
        }
    }

    /// Sets left_mouse_down to false and updates the window if needed.
    /// If the mouse was over the face, restarts the game.
    /// If the mouse was over the grid, sends the click along to the game.
    fn left_mouse_released(&mut self, event_loop: &event_loop::ActiveEventLoop) {
        self.left_mouse_down = false;

        // Store old game state for use in updating face
        let old_game_state = self.game.game_state;

        // Get position on grid and if face is pressed
        let grid_pos = main_window_graphics::convert_to_over_grid(
            self.game.width,
            self.game.height,
            self.cursor_pos,
        );
        let face_pressed =
            main_window_graphics::is_over_face(self.game.width, self.game.height, self.cursor_pos);

        if let Some(pos) = grid_pos { // Try clicking cell on grid
            use minesweeper::{
                CellImage,
                GameState,
            };

            // Start game if before game
            // Set start time to now and set control flow to send an event in 1 second to update the
            // timer
            if let GameState::BeforeGame = self.game.game_state {
                self.game_start_time = std::time::Instant::now();
                event_loop.set_control_flow(event_loop::ControlFlow::WaitUntil(
                    self.game_start_time + std::time::Duration::from_secs_f32(1.0),
                ));
            }

            // Perform the click on the cell, get the list of cells to update, and update the grid
            // using the updates.
            let updates = self.game.left_click(pos);
            self.main_window_graphics.update_grid(&updates);

            // If the game is not running, stops event loop from resuming after the timer
            // No longer need it to resume as the timer should have stopped running
            if !matches!(self.game.game_state, GameState::DuringGame) {
                event_loop.set_control_flow(event_loop::ControlFlow::Wait);
            }

            // If the update was just a change between flagged and question marked, update mines
            // unflagged. It is an invariant that flagged <-> question marked will be the only
            // update when they happen.
            if updates.len() == 1 && let CellImage::Flagged | CellImage::QuestionMarked = updates[0].1 {
                self.update_display(main_window_graphics::Display::MinesUnflagged);
            }

            // If the game was ended by this click, update the mines unflagged display and print the
            // time.
            if !updates.is_empty() && let GameState::Victory = self.game.game_state {
                self.update_display(main_window_graphics::Display::MinesUnflagged);
                let game_duration_ms = self.game_start_time.elapsed().as_millis();
                let game_duration_seconds = game_duration_ms / 1000;
                println!(
                    "Game duration: {}.{} seconds",
                    game_duration_seconds,
                    game_duration_ms % 1000
                );
            }
        } else if face_pressed { // Press face
            // Reset "everything"
            self.game.reset();
            self.main_window_graphics.reset_grid();
            self.game_start_time = std::time::Instant::now();
            self.update_display(main_window_graphics::Display::Timer);
            self.update_display(main_window_graphics::Display::MinesUnflagged);
            event_loop.set_control_flow(event_loop::ControlFlow::Wait);
        }

        // Update face and grid and request redraw
        // Redraw will always be needed to at least update face
        self.update_grid_and_face(self.cursor_pos, true, old_game_state);
        self.window.request_redraw();
    }

    /// If the mouse was over the grid, sends the click along to the game.
    /// Updates the window if needed.
    fn right_mouse_down(&mut self) {
        let grid_pos = main_window_graphics::convert_to_over_grid(
            self.game.width,
            self.game.height,
            self.cursor_pos,
        );
        if let Some(pos) = grid_pos {
            let update = self.game.right_click(pos);
            if let Some(update) = update {
                self.main_window_graphics.update_grid(&[update]);
                self.update_display(main_window_graphics::Display::MinesUnflagged);
                self.window.request_redraw();
            }
        }
    }

    /// Handles user inputs to the window.
    /// Returns whether the event matched any of its cases.
    fn input(&mut self, event: &WindowEvent, event_loop: &event_loop::ActiveEventLoop) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.move_cursor(position);
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.right_mouse_down();
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.left_mouse_down();
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.left_mouse_released(event_loop);
                true
            }
            _ => false,
        }
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
                state.update_display(main_window_graphics::Display::Timer);
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
                std::mem::swap(self, &mut MinesweeperApp::Running(State::new(window, game)));
            }
        }
    }

    fn user_event(&mut self, _event_loop: &event_loop::ActiveEventLoop, _event: ()) {}

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

    fn device_event(
        &mut self,
        _event_loop: &event_loop::ActiveEventLoop,
        _device_id: DeviceId,
        _event: DeviceEvent,
    ) {
    }

    fn about_to_wait(&mut self, _event_loop: &event_loop::ActiveEventLoop) {}

    fn suspended(&mut self, event_loop: &event_loop::ActiveEventLoop) {
        let state = std::mem::replace(self, MinesweeperApp::Suspended(None));
        if let MinesweeperApp::Running(state) = state {
            if let MinesweeperApp::Suspended(game) = self {
                event_loop.set_control_flow(event_loop::ControlFlow::Wait);
                std::mem::swap(game, &mut Some(state.game));
                panic!("Not fully implemented: need to store game start time to be able to resume");
            }
        }
    }

    fn exiting(&mut self, _event_loop: &event_loop::ActiveEventLoop) {}

    fn memory_warning(&mut self, _event_loop: &event_loop::ActiveEventLoop) {}
}

/// Sets up the window and state and runs the event loop.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    env_logger::init();

    // Get starting game params
    let result = starting_params::get_starting_params();

    // In case of error getting params, print error and return
    if let Err(message) = result {
        println!("{}", message);
        return;
    }

    // Destructure starting params and start game
    let (width, height, num_mines) = result.unwrap();
    let event_loop = event_loop::EventLoop::new().unwrap();
    event_loop
        .run_app(&mut MinesweeperApp::Suspended(Some(
            minesweeper::Game::new(width, height, num_mines),
        )))
        .expect("Event loop crashed!");
}
