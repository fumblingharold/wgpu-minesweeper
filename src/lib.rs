mod texture;
mod minesweeper;
mod load_textures;

use std::time::Instant;
use image::GenericImageView;
use cgmath::prelude::*;
use wgpu::util::DeviceExt;
use winit::window::Window;
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

const DEFAULT_WIDTH: u32 = 16;
const DEFAULT_HEIGHT: u32 = 9;
const DEFAULT_MINES: u64 = 20;

const ASPECT_RATIO: winit::dpi::LogicalSize<f32> = winit::dpi::LogicalSize::new(16.0, 9.0);

struct Camera {
    scaling: cgmath::Vector2<f32>,
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

const NUM_INSTANCES_PER_ROW: u32 = 1;
const INSTANCE_DISPLACEMENT: cgmath::Vector3<f32> = cgmath::Vector3::new(
    NUM_INSTANCES_PER_ROW as f32 * 0.5, NUM_INSTANCES_PER_ROW as f32 * 0.5, 0.0);


impl Camera {
    fn rescale(&mut self, win_size: &winit::dpi::PhysicalSize<u32>) {
        let width = win_size.width as f32;
        let height = win_size.height as f32;
        let window_ratio = (width * ASPECT_RATIO.height) / (height * ASPECT_RATIO.width);
        if window_ratio <= 1.0 {
            self.scaling.x = 1.0;
            self.scaling.y = window_ratio;
        } else {
            self.scaling.x = 1.0 / window_ratio;
            self.scaling.y = 1.0;
        }
    }

    fn build_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        [[self.scaling.x, 0.0, 0.0, 0.0],
         [0.0, self.scaling.y, 0.0, 0.0],
         [0.0, 0.0, 1.0, 0.0],
         [0.0, 0.0, 0.0, 1.0]].into()
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_projection_matrix().into();
    }
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    color: wgpu::Color,
    render_pipeline: wgpu::RenderPipeline,
    objects: Vec<texture::Object>,
    camera: Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth_texture: texture::Texture,
    minesweeper_grid: minesweeper::Game,
    cursor_pos: cgmath::Vector2<f64>,
    // The window must be declared after the surface so
    // it gets dropped after it as the surface contains
    // unsafe references to the window's resources.
    window: &'a Window,
}

impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
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

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

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
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result in all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
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

        /*
        let instances = (0..NUM_INSTANCES_PER_ROW).flat_map(|y| {
            (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                let position = cgmath::Vector3 { x: x as f32, y: y as f32, z: 0.0 } - INSTANCE_DISPLACEMENT;

                let rotation = cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0));

                Instance { position, rotation }
            })
        }).collect::<Vec<_>>();
         */

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

        let color = wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.3,
            a: 1.0,
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let camera = Camera {
            scaling: cgmath::Vector2::new(1.0, 1.0),
        };

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let camera_bind_group_layout = device.create_bind_group_layout(
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
                label: Some("Camera_bind Group Layout"),
            }
        );

        let camera_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &camera_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
                label: Some("Camera Binding Group"),
            }
        );

        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
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
                cull_mode: Some(wgpu::Face::Back),
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

        let minesweeper_grid = minesweeper::Game::new(DEFAULT_WIDTH, DEFAULT_HEIGHT, DEFAULT_MINES).unwrap();

        let objects = vec![
            load_textures::get_grid_texture(&device, &queue, &texture_bind_group_layout, DEFAULT_WIDTH, DEFAULT_HEIGHT),
        ];

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            color,
            render_pipeline,
            objects,
            camera,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            cursor_pos: cgmath::Vector2::new(0.0, 0.0),
            minesweeper_grid,
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
            self.camera.rescale(&self.size);
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved {
                device_id: _device_id,
                position,
            } => {
                self.color = wgpu::Color {
                    r: position.x / self.config.width as f64,
                    g: position.y / self.config.height as f64,
                    b: 0.3,
                    a: 1.0,
                };
                false
            },
            _ => false,
        }
    }

    fn update(&mut self) {
        self.queue.write_buffer(&self.camera_buffer, 0,
                                bytemuck::cast_slice(&[CameraUniform {
                                    view_proj: self.camera.build_projection_matrix().into()
                                }])
        )
    }

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
                        load: wgpu::LoadOp::Clear(self.color),
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
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

            for object in &self.objects {
                render_pass.set_bind_group(0, &object.bind_group, &[]);
                render_pass.set_vertex_buffer(0, object.vertex_buffer.slice(..));
                render_pass.set_vertex_buffer(1, object.instance_buffer.slice(..));
                render_pass.set_index_buffer(object.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                //println!("{}", object.instances.len());
                render_pass.draw_indexed(0..object.num_indices, 0, 0..object.instances.len() as _);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(&window).await;

    let start = Instant::now();
    let mut num_redraws: u64 = 0;

    event_loop.run(move |event, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => control_flow.set_control_flow(
                winit::event_loop::ControlFlow::wait_duration(std::time::Duration::from_secs_f32(1.0 / 60.0))),
            Event::NewEvents(StartCause::ResumeTimeReached {start: _, requested_resume}) => {
                control_flow.set_control_flow(
                    winit::event_loop::ControlFlow::WaitUntil(requested_resume + std::time::Duration::from_secs_f32(1.0 / 60.0)));
                state.window.request_redraw();

                //println!("fps: {}", num_redraws as f64 / start.elapsed().as_secs_f64());
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window.id() => if !state.input(event) {
                match event {
                    WindowEvent::RedrawRequested => {
                        num_redraws += 1;

                        state.update();
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
                    WindowEvent::KeyboardInput { event, .. } => {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::KeyR) if event.state.is_pressed() => {
                                state.minesweeper_grid.reset();
                                for (instance, image) in state.objects[0].instances.iter_mut().zip(state.minesweeper_grid.get_all_images().iter().flatten()) {
                                    instance.tex_cord_translation = texture::get_tex_coords(&image);
                                }
                                state.objects[0].update(&state.device, &state.queue);
                                state.window.request_redraw();
                            },
                            _ => {},
                        }
                    },
                    WindowEvent::CursorMoved { position, .. } => {
                        state.cursor_pos.x = position.x / state.size.width  as f64 / state.camera.scaling.x as f64;
                        state.cursor_pos.y =
                            (position.y / state.size.height as f64 + state.camera.scaling.y as f64 / 2.0 - 0.5)
                                / state.camera.scaling.y as f64;
                        //println!("{}", state.cursor_pos.y);
                    },
                    WindowEvent::MouseInput { state: mouse_state, button, .. }
                    if mouse_state.is_pressed() => {
                        if state.cursor_pos.x >= 0.0 && state.cursor_pos.x < 1.0
                            && state.cursor_pos.y >= 0.0 && state.cursor_pos.y < 1.0 {
                            let row = ((1.0 - state.cursor_pos.y) * state.minesweeper_grid.height() as f64) as u32;
                            let col = (state.cursor_pos.x * state.minesweeper_grid.width() as f64) as u32;
                            let grid_object = &mut state.objects[0];
                            let result =
                                if button == &MouseButton::Left {
                                    state.minesweeper_grid.left_click((row, col))
                                } else if button == &MouseButton::Right {
                                    state.minesweeper_grid.right_click((row, col))
                                } else {
                                    Vec::new()
                                };
                            result.iter().for_each(|(row, col, image)| {
                                let index = *row as usize * state.minesweeper_grid.width() as usize + *col as usize;
                                grid_object.instances[index].tex_cord_translation = texture::get_tex_coords(image);
                                grid_object.update_instance(&state.queue, index);
                            });
                        }
                    },
                    _ => {}
                }
            },
            _ => {}
        }
    }).expect("TODO: panic message");
}