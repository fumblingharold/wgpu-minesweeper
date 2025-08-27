use cgmath::num_traits::FromPrimitive;
use std::sync::Arc;
use wgpu::util::DeviceExt;

mod seven_segment;
mod texture;

use crate::minesweeper;
pub use seven_segment::Display;
use seven_segment::{
    DIGITS_PER_DISPLAY,
    DIGIT_HEIGHT,
    DIGIT_WIDTH,
};

/// Hard coded information about the number of pixels in the textures.
pub const KNOWN_FRAME_WIDTHS: [u16; 2] = [12, 8];
pub const KNOWN_FRAME_HEIGHTS: [u16; 4] = [8, 11, 33, 12];
pub const DISPLAY_OFFSET_Y: u16 = (KNOWN_FRAME_HEIGHTS[2] - DIGIT_HEIGHT) / 2;
pub const DISPLAY_OFFSET_X: u16 = DISPLAY_OFFSET_Y - 1;
const DISPLAY_WIDTH: u16 = DIGIT_WIDTH * DIGITS_PER_DISPLAY as u16;
const CELL_LENGTH: u16 = 16;

/// Vertex indices for a square with the above vertices.
const SQUARE_INDICES: &[u16] = &[0, 2, 1, 1, 2, 3];

/// [Vertex]s for a square.
const SQUARE_VERTICES: &[texture::Vertex] = &[
    texture::Vertex {
        position: [0.0, 0.0],
        tex_coords: [0.0, 1.0],
    },
    texture::Vertex {
        position: [0.0, 1.0],
        tex_coords: [0.0, 0.0],
    },
    texture::Vertex {
        position: [1.0, 0.0],
        tex_coords: [1.0, 1.0],
    },
    texture::Vertex {
        position: [1.0, 1.0],
        tex_coords: [1.0, 0.0],
    },
];

/// Handles all graphics for the main window.
pub struct MainWindowGraphics {
    texture_renderer: texture::TextureRenderer,
    rectangles: texture::TextureInstances,
    grid_width: minesweeper::Dim,
    grid_height: minesweeper::Dim,
    scaling: texture::Scaling,
    scaling_buffer: wgpu::Buffer,
    scaling_bind_group: Arc<wgpu::BindGroup>,
    render_pipeline: Arc<wgpu::RenderPipeline>,
}

impl MainWindowGraphics {
    /// Creates a new [MainWindowGraphics] displaying an unstarted minesweeper game with the given
    /// parameters.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_format: wgpu::TextureFormat,
        width: minesweeper::Dim,
        height: minesweeper::Dim,
        mines: minesweeper::Count,
    ) -> Self {
        let texture_layout = make_texture_layout(device);
        let (scaling, scaling_buffer, scaling_layout, scaling_bind_group) =
            make_scaling_items(device);
        let render_pipeline =
            make_render_pipeline(device, texture_format, &texture_layout, &scaling_layout);

        let scaling_bind_group = Arc::new(scaling_bind_group);
        let render_pipeline = Arc::new(render_pipeline);

        let diffuse_bytes = include_bytes!("atlas.png");
        let texture =
            texture::from_bytes(&device, &queue, diffuse_bytes, Some("Rectangles Texture"))
                .expect("Failed to load Frame Texture");
        let texture_renderer = texture::TextureRenderer::new(
            device,
            render_pipeline.clone(),
            scaling_bind_group.clone(),
            &texture_layout,
            "Rectangles Texture".parse().unwrap(),
            texture,
            SQUARE_INDICES,
            &[],
            SQUARE_VERTICES,
        );

        let mut result = Self {
            texture_renderer,
            rectangles: texture::TextureInstances::new(Vec::new()),
            grid_width: width,
            grid_height: height,
            scaling,
            scaling_buffer,
            scaling_bind_group,
            render_pipeline,
        };
        let rectangles = get_main_window_instances(&result, mines);
        result.rectangles.set_instances(rectangles);
        result
    }

    /// Returns the x component of the scaling array.
    pub fn scaling_x(&self) -> f32 {
        self.scaling.scaling.x
    }

    /// Returns the y component of the scaling array.
    pub fn scaling_y(&self) -> f32 {
        self.scaling.scaling.y
    }

    /// Updates the scaling array based on the new window size.
    pub fn rescale(&mut self, size: &winit::dpi::PhysicalSize<u32>) {
        self.scaling.rescale(
            size,
            get_total_pixel_width(self.grid_width) as f32,
            get_total_pixel_height(self.grid_height) as f32,
        );
    }

    /// Renders the graphics to the given [wgpu::RenderPass].
    pub fn render(
        &mut self,
        render_pass: &mut wgpu::RenderPass,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            &self.scaling_buffer,
            0,
            bytemuck::cast_slice(&[texture::ScalingUniform::new(&self.scaling)]),
        );
        self.texture_renderer
            .prepare(self.rectangles.get_data(), device, queue);
        self.texture_renderer.render(render_pass);
    }

    /// Resets all cells in the grid to be hidden.
    pub fn reset_grid(&mut self) {
        let num_cells = self.grid_width as usize * self.grid_height as usize;
        let grid_start_index = 6 + 15;
        let grid_end_index = grid_start_index + num_cells;
        let tex_coord_translation = self.get_tex_trans(
            get_cell_tex_coords_new(&minesweeper::CellImage::Hidden),
            0,
            0,
        );
        (grid_start_index..grid_end_index).for_each(|idx| {
            self.rectangles
                .update_tex_coord_instance(idx, tex_coord_translation)
        });
    }

    /// Updates all cells as described.
    pub fn update_grid(&mut self, updates: Vec<(minesweeper::Pos, minesweeper::CellImage)>) {
        updates.iter().for_each(|((row, col), cell_image)| {
            let index = 6 + 15 + (*col as usize + *row as usize * self.grid_width as usize);
            let tex_coord_translation =
                self.get_tex_trans(get_cell_tex_coords_new(cell_image), 0, 0);
            self.rectangles
                .update_tex_coord_instance(index, tex_coord_translation);
        });
    }

    /// Updates the given [Display] with the given value.
    pub fn update_display(&mut self, display: seven_segment::Display, val: i32) {
        let is_timer = match display {
            Display::MinesUnflagged => false,
            Display::Timer => true,
        };
        let updated_digits = seven_segment::get_texture_coords(val);
        let offset = if is_timer { DIGITS_PER_DISPLAY } else { 0 };
        let updated_tex_coords = updated_digits
            .into_iter()
            .map(|data| self.get_tex_trans(data, -64, 0))
            .collect::<Vec<_>>()
            .into_iter()
            .zip(0..DIGITS_PER_DISPLAY);
        for (data, idx) in updated_tex_coords {
            self.rectangles
                .update_tex_coord_instance(15 + idx + offset, data);
        }
    }

    /// Creates a texture coordinate translation array using the given data and the data within this
    /// [MainWindowGraphics].
    fn get_tex_trans(&self, tex_translation: [u16; 2], x_offset: i32, y_offset: i32) -> [f32; 2] {
        let tex_coord_translation = [tex_translation[0] as f32, tex_translation[1] as f32];
        let offset = [x_offset as f32, y_offset as f32];
        let scaling = [
            self.texture_renderer.atlas_width() as f32,
            self.texture_renderer.atlas_height() as f32,
        ];
        Self::scale_data(tex_coord_translation, offset, scaling)
    }

    fn scale_data(data: [f32; 2], offset: [f32; 2], scaling: [f32; 2]) -> [f32; 2] {
        [
            (data[0] - offset[0]) / scaling[0],
            (data[1] - offset[1]) / scaling[1],
        ]
    }

    /// Creates a [texture::Instance] using the given data and the data within this
    /// [MainWindowGraphics].
    fn instance_from_pixel_data(
        &self,
        vertex_translation: [u16; 2],
        vertex_scale: [u16; 2],
        tex_coord_translation: [u16; 2],
        tex_coord_scale: [u16; 2],
        x_offset: u16,
        y_offset: u16,
    ) -> texture::Instance {
        assert!(
            tex_coord_translation[0] + tex_coord_scale[0] - 1 < self.texture_renderer.atlas_width()
                && tex_coord_translation[1] + tex_coord_scale[1] - 1
                    < self.texture_renderer.atlas_height(),
            "Texture coordinates out of bounds"
        );
        let to_f32 = |array: [u16; 2]| [array[0] as f32, array[1] as f32];
        let vertex_translation = to_f32(vertex_translation);
        let vertex_scale = to_f32(vertex_scale);
        let tex_coord_translation = to_f32(tex_coord_translation);
        let tex_coord_scale = to_f32(tex_coord_scale);

        let vertex_translation_offset = to_f32([
            get_total_pixel_width(self.grid_width) / 2,
            get_total_pixel_height(self.grid_height) / 2,
        ]);
        let vertex_scaling_offset = [0.0, 0.0];
        let tex_coord_translation_offset = [-1.0 * x_offset as f32, -1.0 * y_offset as f32];
        let tex_coord_scaling_offset = [0.002, 0.002];
        let vertex_data_scaling = vertex_translation_offset;
        let tex_coord_scaling = to_f32([
            self.texture_renderer.atlas_width(),
            self.texture_renderer.atlas_height(),
        ]);

        let vertex_translation = Self::scale_data(
            vertex_translation,
            vertex_translation_offset,
            vertex_data_scaling,
        );
        let vertex_scaling =
            Self::scale_data(vertex_scale, vertex_scaling_offset, vertex_data_scaling);
        let tex_coord_translation = Self::scale_data(
            tex_coord_translation,
            tex_coord_translation_offset,
            tex_coord_scaling,
        );
        let tex_coord_scaling =
            Self::scale_data(tex_coord_scale, tex_coord_scaling_offset, tex_coord_scaling);
        texture::Instance::new(
            vertex_translation,
            vertex_scaling,
            tex_coord_translation,
            tex_coord_scaling,
        )
    }
}

/// Returns the width of the minesweeper game in pixels given the grid's width.
fn get_total_pixel_width(width: minesweeper::Dim) -> u16 {
    width as u16 * CELL_LENGTH + KNOWN_FRAME_WIDTHS.iter().sum::<u16>()
}

/// Returns the height of the minesweeper game in pixels given the grid's height.
fn get_total_pixel_height(height: minesweeper::Dim) -> u16 {
    height as u16 * CELL_LENGTH + KNOWN_FRAME_HEIGHTS.iter().sum::<u16>()
}

/// Rescaled and translates a position on the image to be relative to the grid.
pub fn convert_to_over_grid(
    width: minesweeper::Dim,
    height: minesweeper::Dim,
    pos: cgmath::Vector2<f32>,
) -> Option<minesweeper::Pos> {
    let to_u8_on_grid = |pos, length, offset| -> Option<u8> {
        u8::from_f32(((pos + 1.0) / 2.0 * length as f32 - offset as f32) / CELL_LENGTH as f32)
    };
    let col = to_u8_on_grid(pos.x, get_total_pixel_width(width), KNOWN_FRAME_WIDTHS[0])?;
    let row = to_u8_on_grid(
        pos.y,
        get_total_pixel_height(height),
        KNOWN_FRAME_HEIGHTS[0],
    )?;
    if row < height && col < width {
        Some((row, col))
    } else {
        None
    }
}

/// Creates the initial [texture::Instance]s.
fn get_main_window_instances(
    main_window_graphics: &MainWindowGraphics,
    mines: minesweeper::Count,
) -> Vec<texture::Instance> {
    let grid_width = main_window_graphics.grid_width;
    let grid_height = main_window_graphics.grid_height;
    let mut instances = Vec::with_capacity(
        15 + DIGITS_PER_DISPLAY * 2 + (grid_width as usize * grid_height as usize),
    );

    // Create instance data for the border
    let mut vtx = [0, KNOWN_FRAME_WIDTHS[0], CELL_LENGTH * grid_width as u16];
    let mut vty = [
        0,
        KNOWN_FRAME_HEIGHTS[0],
        CELL_LENGTH * grid_height as u16,
        KNOWN_FRAME_HEIGHTS[1],
        KNOWN_FRAME_HEIGHTS[2],
    ];
    let mut vsx = [
        KNOWN_FRAME_WIDTHS[0],
        CELL_LENGTH * grid_width as u16,
        KNOWN_FRAME_WIDTHS[1],
    ];
    let mut vsy = [
        KNOWN_FRAME_HEIGHTS[0],
        CELL_LENGTH * grid_height as u16,
        KNOWN_FRAME_HEIGHTS[1],
        KNOWN_FRAME_HEIGHTS[2],
        KNOWN_FRAME_HEIGHTS[3],
    ];
    let mut ttx = [0, KNOWN_FRAME_WIDTHS[0], 1];
    let mut tty = [0, KNOWN_FRAME_HEIGHTS[3], 1, KNOWN_FRAME_HEIGHTS[1], 1];
    let mut tsx = [KNOWN_FRAME_WIDTHS[0], 1, KNOWN_FRAME_WIDTHS[1]];
    let mut tsy = [
        KNOWN_FRAME_HEIGHTS[3],
        1,
        KNOWN_FRAME_HEIGHTS[1],
        1,
        KNOWN_FRAME_HEIGHTS[0],
    ];
    for idx in 1..vtx.len() {
        vtx[idx] = vtx[idx - 1] + vtx[idx];
    }
    for idx in 1..vty.len() {
        vty[idx] = vty[idx - 1] + vty[idx];
    }
    for idx in 1..ttx.len() {
        ttx[idx] = ttx[idx - 1] + ttx[idx];
    }
    for idx in 1..tty.len() {
        tty[idx] = tty[idx - 1] + tty[idx];
    }
    vty.reverse();
    vsy.reverse();
    for ((vty, vsy), (tty, tsy)) in vty.iter().zip(vsy.iter()).zip(tty.iter().zip(tsy.iter())) {
        for ((vtx, vsx), (ttx, tsx)) in vtx.iter().zip(vsx.iter()).zip(ttx.iter().zip(tsx.iter())) {
            instances.push(main_window_graphics.instance_from_pixel_data(
                [*vtx, *vty],
                [*vsx, *vsy],
                [*ttx, *tty],
                [*tsx, *tsy],
                95,
                69,
            ));
        }
    }

    // Create instance data for displays
    let mines_left_digits = seven_segment::get_texture_coords(mines as i32).into_iter();
    let timer_digits = seven_segment::get_texture_coords(0).into_iter();
    let mut digits = mines_left_digits.chain(timer_digits);
    let vertex_scale = [DIGIT_WIDTH, DIGIT_HEIGHT];
    let y = KNOWN_FRAME_HEIGHTS[0]
        + CELL_LENGTH * grid_height as u16
        + KNOWN_FRAME_HEIGHTS[1]
        + DISPLAY_OFFSET_Y;
    let left_side_xs = [
        KNOWN_FRAME_WIDTHS[0] + DISPLAY_OFFSET_X,
        KNOWN_FRAME_WIDTHS[0] + CELL_LENGTH * grid_width as u16 - DISPLAY_OFFSET_X - DISPLAY_WIDTH,
    ];
    for left_side_x in left_side_xs.iter() {
        for digit in 0..DIGITS_PER_DISPLAY {
            instances.push(main_window_graphics.instance_from_pixel_data(
                [left_side_x + DIGIT_WIDTH * digit as u16, y],
                vertex_scale,
                digits.next().unwrap(),
                [13, 23],
                64,
                0,
            ));
        }
    }

    // Create instance data for grid
    let tex_coord_translation = get_cell_tex_coords_new(&minesweeper::CellImage::Hidden);
    instances.append(
        &mut (0..grid_height as u16)
            .flat_map(|row| {
                (0..grid_width as u16).map(move |col| {
                    main_window_graphics.instance_from_pixel_data(
                        [
                            KNOWN_FRAME_WIDTHS[0] + col * CELL_LENGTH,
                            KNOWN_FRAME_HEIGHTS[0] + row * CELL_LENGTH,
                        ],
                        [CELL_LENGTH, CELL_LENGTH],
                        tex_coord_translation,
                        [CELL_LENGTH, CELL_LENGTH],
                        0,
                        0,
                    )
                })
            })
            .collect::<Vec<_>>(),
    );

    instances
}

/// Returns the texture coordinates for the given [CellImage]. This is based on the texture atlas in
/// Grid.png.
fn get_cell_tex_coords_new(image: &minesweeper::CellImage) -> [u16; 2] {
    use minesweeper::CellImage::*;
    match image {
        Zero => [0 * CELL_LENGTH, 0 * CELL_LENGTH],
        One => [1 * CELL_LENGTH, 0 * CELL_LENGTH],
        Two => [2 * CELL_LENGTH, 0 * CELL_LENGTH],
        Three => [3 * CELL_LENGTH, 0 * CELL_LENGTH],
        Four => [0 * CELL_LENGTH, 1 * CELL_LENGTH],
        Five => [1 * CELL_LENGTH, 1 * CELL_LENGTH],
        Six => [2 * CELL_LENGTH, 1 * CELL_LENGTH],
        Seven => [3 * CELL_LENGTH, 1 * CELL_LENGTH],
        Eight => [0 * CELL_LENGTH, 2 * CELL_LENGTH],
        Mine => [1 * CELL_LENGTH, 2 * CELL_LENGTH],
        WronglyFlagged => [2 * CELL_LENGTH, 2 * CELL_LENGTH],
        SelectedMine => [3 * CELL_LENGTH, 2 * CELL_LENGTH],
        Hidden => [0, 3 * CELL_LENGTH],
        Flagged => [0, 4 * CELL_LENGTH],
        QuestionMarked => [0, 5 * CELL_LENGTH],
    }
}

/// Creates a [wgpu::BindGroupLayout] for textures.
fn make_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    // Create a bind group layout for the grid
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
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
        label: Some("Texture Bind Group Layout"),
    })
}

/// Creates all scaling items required for rendering.
fn make_scaling_items(
    device: &wgpu::Device,
) -> (
    texture::Scaling,
    wgpu::Buffer,
    wgpu::BindGroupLayout,
    wgpu::BindGroup,
) {
    let scaling = texture::Scaling {
        scaling: cgmath::Vector2::new(1.0, 1.0),
    };

    let scaling_uniform = texture::ScalingUniform::new(&scaling);
    let scaling_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Scaling Buffer"),
        contents: bytemuck::cast_slice(&[scaling_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let scaling_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        });

    let scaling_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &scaling_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: scaling_buffer.as_entire_binding(),
        }],
        label: Some("Camera Binding Group"),
    });

    (
        scaling,
        scaling_buffer,
        scaling_bind_group_layout,
        scaling_bind_group,
    )
}

/// Creates the [wgpu::RenderPipeline] for rendering.
/// Should be moved into texture.rs as it is the same for all [texture::TextureRenderer]s.
fn make_render_pipeline(
    device: &wgpu::Device,
    texture_format: wgpu::TextureFormat,
    texture_layout: &wgpu::BindGroupLayout,
    scaling_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    // Create a handle for the shader file
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[texture_layout, scaling_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[texture::Vertex::desc(), texture::Instance::desc()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: texture_format,
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
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}
