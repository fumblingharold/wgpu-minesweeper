mod texture;
mod minesweeper;
mod load_textures;
mod seven_segment;

use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::window::Window;
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};
use winit::event_loop::EventLoopWindowTarget;

const DEFAULT_WIDTH: minesweeper::Dim = 9;
const DEFAULT_HEIGHT: minesweeper::Dim = 9;
const DEFAULT_MINES: minesweeper::Count = 10;

/// Stores info on how to scale each instance to fit the window as an x-scaling and a y-scaling.
struct Scaling {
    scaling: cgmath::Vector2<f32>,
}

impl Scaling {
    /// Updates the camera based on the given window size and game aspect ratio.
    fn rescale(&mut self, win_size: &winit::dpi::PhysicalSize<u32>, aspect_ratio_height: f32, aspect_ratio_width: f32) {
        let width = win_size.width as f32;
        let height = win_size.height as f32;
        let window_ratio = (width * aspect_ratio_height) / (height * aspect_ratio_width);
        // If the window is too tall for the aspect ratio, scale the x to fit the window and y to keep aspect ratio.
        // Otherwise, scale the y to fit the window and x to keep the aspect ratio.
        if window_ratio <= 1.0 {
            self.scaling.x = 1.0;
            self.scaling.y = window_ratio;
        } else {
            self.scaling.x = 1.0 / window_ratio;
            self.scaling.y = 1.0;
        }
    }

    /// Build a scaling matrix using the given camera.
    fn build_scaling_matrix(&self) -> cgmath::Matrix4<f32> {
        [[self.scaling.x, 0.0, 0.0, 0.0],
         [0.0, self.scaling.y, 0.0, 0.0],
         [0.0, 0.0, 1.0, 0.0],
         [0.0, 0.0, 0.0, 1.0]].into()
    }
}

/// Stores info on how to scale each instance to fit the window as a 4x4 scaling matrix.
/// Uses #[repr(C)] for wgsl shader compatability.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ScalingUniform {
    scaling: [[f32; 4]; 4],
}

impl ScalingUniform {

    /// Creates a new CameraUniform from the given Camera.
    fn new(camera: &Scaling) -> Self {
        use cgmath::SquareMatrix;
        let mut result = Self { scaling: cgmath::Matrix4::identity().into() };
        result.update(camera);
        result
    }

    /// Updates the CameraUniform using the given Camera.
    fn update(&mut self, camera: &Scaling) {
        self.scaling = camera.build_scaling_matrix().into();
    }
}

/// The State of a  Minesweeper game process.
struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    objects: texture::Objects,
    scaling: Scaling,
    scaling_buffer: wgpu::Buffer,
    scaling_bind_group: wgpu::BindGroup,
    depth_texture: texture::Texture,
    minesweeper_grid: minesweeper::Game,
    cursor_pos: cgmath::Vector2<f64>,
    seconds_since_game_start: u32,
    // The window must be declared after the surface so
    // it gets dropped after it as the surface contains
    // unsafe references to the window's resources.
    window: &'a Window,
}

impl<'a> State<'a> {
    /// Creates a new State.
    /// It is async as creating some of the wgpu types requires async code.
    async fn new(window: &'a Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch="wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch="wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        // Handle for the window
        let surface = instance.create_surface(window).unwrap();

        // Adapter for instance
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        // Adapter provides device for allocating GPU memory and queue editing GPU memory
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
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
            },
            None, // Trace path
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        // Shader code uses sRGB surface textures
        let surface_format = surface_caps.formats.iter()
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

        // Create a bind group layout for the grid
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("Texture Bind Group Layout"),
            });

        // Create a handle for the shader file
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Scaling of textures
        let scaling = Scaling { scaling: cgmath::Vector2::new(1.0, 1.0) };

        // GPU friendly form of Scaling
        let scaling_uniform = ScalingUniform::new(&scaling);
        let scaling_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Scaling Buffer"),
                contents: bytemuck::cast_slice(&[scaling_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let scaling_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("Scaling Bind Group Layout"),
            }
        );
        let scaling_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &scaling_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: scaling_buffer.as_entire_binding(),
                }],
                label: Some("Camera Binding Group"),
            }
        );

        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &scaling_bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[texture::Vertex::desc(), texture::InstanceRaw::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Set up the game with default values
        let minesweeper_grid = minesweeper::Game::new(DEFAULT_WIDTH, DEFAULT_HEIGHT, DEFAULT_MINES);

        // Set up textures for grid
        let objects = texture::Objects::new(&device, &queue, &texture_bind_group_layout,
                                            DEFAULT_WIDTH, DEFAULT_HEIGHT, DEFAULT_MINES);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            objects,
            scaling,
            scaling_buffer,
            scaling_bind_group,
            depth_texture,
            cursor_pos: cgmath::Vector2::new(0.0, 0.0),
            minesweeper_grid,
            seconds_since_game_start: 0,
        }
    }

    /// Handles updating the State with a new window size.
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.scaling.rescale(&self.size, self.minesweeper_grid.height as f32 + 64.0 / 16.0,
                                 self.minesweeper_grid.width as f32 + 21.0 / 16.0);
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        }
    }

    /// Handles user inputs to the window.
    fn input(&mut self, event: &WindowEvent, control_flow: &EventLoopWindowTarget<()>) -> bool {
        let flags = self.minesweeper_grid.flags;
        let game_state = self.minesweeper_grid.game_state.clone();
        let result = match event {
            WindowEvent::KeyboardInput { event, .. } => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyR) if event.state.is_pressed() => {
                        self.minesweeper_grid.reset();
                        for (instance, image) in self.objects.grid.instances.iter_mut().zip(self.minesweeper_grid.get_all_images().iter().flatten()) {
                            instance.tex_cord_translation = texture::get_cell_tex_coords(&image);
                        }
                        self.objects.grid.update(&self.device, &self.queue);
                        self.window.request_redraw();
                        true
                    },
                    _ => false,
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos.x =
                    (position.x / self.size.width as f64 + self.scaling.scaling.x as f64 / 2.0 - 0.5)
                        / self.scaling.scaling.x as f64;
                self.cursor_pos.y =
                    (position.y / self.size.height as f64 + self.scaling.scaling.y as f64 / 2.0 - 0.5)
                        / self.scaling.scaling.y as f64;
                true
            },
            WindowEvent::MouseInput { state: mouse_state, button, .. }
            if mouse_state.is_pressed() => {
                self.cursor_pos = load_textures::convert_to_over_grid(self.minesweeper_grid.width, self.minesweeper_grid.height, self.cursor_pos);
                if self.cursor_pos.x >= 0.0 && self.cursor_pos.x < 1.0
                    && self.cursor_pos.y >= 0.0 && self.cursor_pos.y < 1.0 {
                    let row = ((1.0 - self.cursor_pos.y) * self.minesweeper_grid.height as f64) as u8;
                    let col = (self.cursor_pos.x * self.minesweeper_grid.width as f64) as u8;
                    let grid_object = &mut self.objects.grid;
                    let result =
                        if button == &MouseButton::Left {
                            self.minesweeper_grid.left_click((row, col))
                        } else if button == &MouseButton::Right {
                            self.minesweeper_grid.right_click((row, col))
                        } else {
                            Vec::new()
                        };
                    result.into_iter().for_each(|((row, col), image)| {
                        let index = row as usize * self.minesweeper_grid.width as usize + col as usize;
                        grid_object.instances[index].tex_cord_translation = texture::get_cell_tex_coords(&image);
                        grid_object.update_instance(&self.queue, index);
                    });
                    self.window.request_redraw();
                    true
                } else {
                    false
                }
            },
            _ => false,
        };
        // If the grid changed, check if displays need to be updated
        if result {
            if flags != self.minesweeper_grid.flags {
                let new_val = self.minesweeper_grid.total_mines as i32 - self.minesweeper_grid.flags as i32;
                self.objects.update_display(seven_segment::Display::MinesUnflagged, new_val, &self.queue);
            }
            if game_state != self.minesweeper_grid.game_state {
                use minesweeper::GameState::*;
                control_flow.set_control_flow(
                    match self.minesweeper_grid.game_state {
                        BeforeGame => {
                            self.seconds_since_game_start = 0;
                            self.objects.update_display(seven_segment::Display::Timer, self.seconds_since_game_start as i32, &self.queue);
                            self.window.request_redraw();
                            winit::event_loop::ControlFlow::Wait
                        },
                        DuringGame => winit::event_loop::ControlFlow::WaitUntil(
                            Instant::now() + std::time::Duration::from_secs_f32(1.0)),
                        AfterGame => winit::event_loop::ControlFlow::Wait,
                    });
            }
        };
        result
    }

    /// Update the scaling_buffer in the GPU using the ScalingUniform.
    fn update(&mut self) {
        self.queue.write_buffer(&self.scaling_buffer, 0,
                                bytemuck::cast_slice(&[ScalingUniform {
                                    scaling: self.scaling.build_scaling_matrix().into()
                                }])
        )
    }

    /// Render the game to the window.
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(1, &self.scaling_bind_group, &[]);

            let objects = self.objects.get_objects();

            for object in objects {
                render_pass.set_bind_group(0, &object.bind_group, &[]);
                render_pass.set_vertex_buffer(0, object.vertex_buffer.slice(..));
                render_pass.set_vertex_buffer(1, object.instance_buffer.slice(..));
                render_pass.set_index_buffer(object.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..object.num_indices, 0, 0..object.instances.len() as _);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

/// Sets up the window and state and runs the event loop.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(&window).await;

    event_loop.run(move |event, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => { state.window.set_title("Minesweeper"); },
            Event::NewEvents(StartCause::ResumeTimeReached {start: _, requested_resume}) => {
                state.seconds_since_game_start += 1;
                control_flow.set_control_flow(
                    winit::event_loop::ControlFlow::WaitUntil(requested_resume + std::time::Duration::from_secs_f32(1.0)));
                state.objects.update_display(seven_segment::Display::Timer, state.seconds_since_game_start as i32, &state.queue);
                state.window.request_redraw();
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window.id() => if !state.input(event, &control_flow) {
                match event {
                    WindowEvent::RedrawRequested => {
                        match state.render() {
                            Ok(_) => {},
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => state.resize(state.size),
                            Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                log::error!("Out of memory");
                                control_flow.exit();
                            },
                        }
                    }
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                        state.update();
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
                    } => control_flow.exit(),
                    _ => {}
                }
            },
            _ => {}
        }
    }).expect("TODO: panic message");
}