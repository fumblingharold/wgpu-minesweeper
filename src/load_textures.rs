use cgmath::num_traits::{FromPrimitive};
use wgpu::util::DeviceExt;
use crate::{minesweeper, texture, seven_segment};

/// Hard coded information about the number of pixels in the textures.
const FRAME_WIDTHS: [f32; 3] = [0.0, 12.0, 8.0];
const FRAME_HEIGHTS: [f32; 5] = [0.0, 8.0, 11.0, 33.0, 12.0];
const DIGIT_WIDTH: f32 = 13.0;
const DIGIT_HEIGHT: f32 = 21.0;
const DISPLAY_OFFSET_Y: f32 = (FRAME_HEIGHTS[3] - DIGIT_HEIGHT) / 2.0;
const DISPLAY_OFFSET_X: f32 = DISPLAY_OFFSET_Y - 1.0;
const DIGITS_PER_DISPLAY: usize = 3;
const DISPLAY_WIDTH: f32 = DIGIT_WIDTH * DIGITS_PER_DISPLAY as f32;
const GRID_LENGTH: f32 = 16.0;

/// Returns the width of the minesweeper game in pixels given the grid's width.
pub fn get_total_pixel_width(width: minesweeper::Dim) -> f32 {
    width as f32 * GRID_LENGTH + FRAME_WIDTHS.iter().sum::<f32>()
}

/// Returns the height of the minesweeper game in pixels given the grid's height.
pub fn get_total_pixel_height(height: minesweeper::Dim) -> f32 {
    height as f32 * GRID_LENGTH + FRAME_HEIGHTS.iter().sum::<f32>()
}

/// Rescaled and translates a position on the image to be relative to the grid.
pub fn convert_to_over_grid<>(width: minesweeper::Dim, height: minesweeper::Dim,
                            pos: cgmath::Vector2<f32>) -> Option<minesweeper::Pos> {
    let to_u8_on_grid = |pos, length, offset | -> Option<u8>{
        u8::from_f32(((pos + 1.0) / 2.0 * length - offset) / GRID_LENGTH)
    };
    let col = to_u8_on_grid(pos.x, get_total_pixel_width(width), FRAME_WIDTHS[1])?;
    let row = to_u8_on_grid(pos.y, get_total_pixel_height(height), FRAME_HEIGHTS[1])?;
    if row < height && col < width {
        Some((row, col))
    } else {
        None
    }
}

/// [Vertex]s for a cell in minesweeper.
const GRID_VERTICES: &[texture::Vertex] = &[
    texture::Vertex { position: [0.0, 0.0, 0.0], tex_coords: [0.0 , 0.25], }, // A
    texture::Vertex { position: [0.0, 1.0, 0.0], tex_coords: [0.0 , 0.0 ], }, // B
    texture::Vertex { position: [1.0, 0.0, 0.0], tex_coords: [0.25, 0.25], }, // C
    texture::Vertex { position: [1.0, 1.0, 0.0], tex_coords: [0.25, 0.0 ], }, // D
];

/// Indices for a cell in minesweeper.
const GRID_INDICES: &[u16] = &[
    0, 2, 1,
    1, 2, 3,
];

/// Creates a [texture::Object] for the minesweeper grid.
pub fn get_grid_texture(device: &wgpu::Device, queue: &wgpu::Queue,
                        bind_group_layout: &wgpu::BindGroupLayout,
                        width: minesweeper::Dim, height: minesweeper::Dim) -> texture::Object {
    // Load grid textures into memory and create a Texture from it
    let diffuse_bytes = include_bytes!("Grid.png");

    // Create instance data
    let half_total_pixel_width = get_total_pixel_width(width) / 2.0;
    let half_total_pixel_height = get_total_pixel_height(height) / 2.0;
    let tex_cord_translation = texture::get_cell_tex_coords(&minesweeper::CellImage::Hidden);
    let instances = (0..height).flat_map(|row| {
        (0..width).map(move |col| {
            texture::Instance {
                vertex_translation: [
                    (FRAME_WIDTHS[1] + col as f32 * GRID_LENGTH) / half_total_pixel_width - 1.0,
                    (FRAME_HEIGHTS[1] + row as f32 * GRID_LENGTH) / half_total_pixel_height - 1.0],
                vertex_scale: [GRID_LENGTH / half_total_pixel_width, GRID_LENGTH / half_total_pixel_height],
                tex_cord_translation,
            }
        })
    }).collect::<Vec<_>>();

    build_texture(device, queue, bind_group_layout, "Grid".parse().unwrap(), diffuse_bytes, GRID_INDICES, instances, GRID_VERTICES)
}

/// Creates a [texture::Object] for the minesweeper border.
/// Needs refactoring
pub fn get_border_texture(device: &wgpu::Device, queue: &wgpu::Queue,
                          bind_group_layout: &wgpu::BindGroupLayout,
                          width: minesweeper::Dim, height: minesweeper::Dim) -> texture::Object {
    // Load grid textures into memory and create a Texture from it
    let image_data = include_bytes!("Frame.png");

    // Create index data
    let mut frame_indices: Vec<u16> = Vec::with_capacity(30);
    let vertices_in_row = 4;
    for x in 0..FRAME_WIDTHS.len() as u16 {
        for y in 0..FRAME_HEIGHTS.len() as u16 {
            if x != 1 || y != 3 {
                let top_lft = x + y * vertices_in_row;
                let top_rgt = top_lft + 1;
                let bot_lft = top_lft + vertices_in_row;
                let bot_rgt = bot_lft + 1;
                frame_indices.append(&mut vec!(bot_lft, top_rgt, top_lft, bot_rgt, top_rgt, bot_lft));
            }
        }
    }
    let frame_indices= frame_indices.as_slice();

    // Create instance data]
    let vertex_scale = [2.0 / get_total_pixel_width(width), 2.0 / get_total_pixel_height(height)];
    let instances = vec!(texture::Instance {
        vertex_translation: [-1.0, -1.0],
        vertex_scale,
        tex_cord_translation: [0.0, 0.0],
    });

    // Create texture data
    // Hardcoded based on texture atlas]
    let tx = vec!(0.0, 12.0 / 21.0, 13.0 / 21.0, 1.0);
    let ty = vec!(0.0, 12.0 / 33.0, 13.0 / 33.0, 24.0 / 33.0, 25.0 / 33.0, 1.0);
    let mut sx = FRAME_WIDTHS.to_vec();
    let mut sy = FRAME_HEIGHTS.to_vec();
    sx.insert(2, width  as f32 * GRID_LENGTH);
    sy.insert(2, height as f32 * GRID_LENGTH);
    for idx in 1..sx.len() {
        sx[idx] = sx[idx - 1] + sx[idx];
    }
    for idx in 1..sy.len() {
        sy[idx] = sy[idx - 1] + sy[idx];
    }
    let (sx, sy) = (sx, sy);
    let mut frame_vertices = Vec::with_capacity(tx.len() * ty.len());
    for (ty, sy) in ty.iter().zip(sy.iter().rev()) {
        for (tx, sx) in tx.iter().zip(sx.iter()) {
            frame_vertices.push(texture::Vertex {
                position: [*sx, *sy, 0.0],
                tex_coords: [*tx, *ty]
            });
        }
    }
    let frame_vertices = frame_vertices.as_slice();

    let name = "Frame".parse().unwrap();

    build_texture(device, queue, bind_group_layout, name, image_data, frame_indices, instances, frame_vertices)
}

/// Creates a [texture::Object] for the minesweeper numbers.
pub fn get_number_texture(device: &wgpu::Device, queue: &wgpu::Queue,
                          bind_group_layout: &wgpu::BindGroupLayout,
                          width: minesweeper::Dim, height: minesweeper::Dim,
                          mines: minesweeper::Count) -> texture::Object {
    // Load grid textures into memory and create a Texture from it
    let image_data = include_bytes!("Numbers.png");

    // Create index data
    let indices = [0, 2, 1, 1, 2, 3];

    // Create instance data
    let half_pixel_width = get_total_pixel_width(width) / 2.0;
    let half_pixel_height = get_total_pixel_height(height) / 2.0;
    let vertex_scale = [DIGIT_WIDTH / half_pixel_width, DIGIT_HEIGHT / half_pixel_height];
    let y = (half_pixel_height * 2.0 - FRAME_HEIGHTS[4] - DISPLAY_OFFSET_Y - DIGIT_HEIGHT) / half_pixel_height - 1.0;
    let left_side_xs = [FRAME_WIDTHS[1] + DISPLAY_OFFSET_X,
        half_pixel_width * 2.0 - FRAME_WIDTHS[2] - DISPLAY_OFFSET_X - DISPLAY_WIDTH];
    let mines_left_digits = seven_segment::get_texture_coords(mines as i32).into_iter();
    let timer_digits = seven_segment::get_texture_coords(0).into_iter();
    let mut digits = mines_left_digits.chain(timer_digits);
    let mut instances = Vec::with_capacity(DIGITS_PER_DISPLAY * 2);
    for left_side_x in left_side_xs.iter() {
        for digit in 0..DIGITS_PER_DISPLAY {
            instances.push(texture::Instance {
                vertex_scale,
                vertex_translation: [(left_side_x + DIGIT_WIDTH * digit as f32) / half_pixel_width - 1.0, y],
                tex_cord_translation: digits.next().unwrap(),
            });
        }
    }
    let instances = instances;

    // Create texture data
    // Hardcoded based on texture atlas
    let texture_xs = vec!(0.0, 1.0);
    let texture_ys = vec!(0.0, 1.0 / 12.0);
    let position_xs = vec!(0.0, 1.0);
    let position_ys = vec!(0.0, 1.0);
    let mut vertices = Vec::with_capacity(texture_xs.len() * texture_ys.len());
    for (texture_x, position_x) in texture_xs.iter().zip(position_xs.iter()) {
        for (texture_y, position_y) in texture_ys.iter().clone().zip(position_ys.iter()) {
            vertices.push(texture::Vertex {
                position: [*position_x, *position_y, 0.0],
                tex_coords: [*texture_x, *texture_y]
            });
        }
    }

    let name = "Numbers".parse().unwrap();

    build_texture(device, queue, bind_group_layout, name, image_data, &indices, instances, &vertices)
}

/// Creates a [texture::Object].
fn build_texture(device: &wgpu::Device, queue: &wgpu::Queue,
                     bind_group_layout: &wgpu::BindGroupLayout, name: String, image_data: &[u8],
                     indices: &[u16], instances: Vec<texture::Instance>, vertices: &[texture::Vertex]) -> texture::Object {
    // Load grid textures into memory and create a Texture from it
    let texture = texture::Texture::from_bytes(&device, &queue, image_data, Some(&*(name.clone() + " Texture")))
        .expect("Failed to load Frame Texture");

    // Get get number of indices
    let num_indices = indices.len() as u32;

    // Create instance data
    let instance_data = instances.iter().map(texture::Instance::to_raw).collect::<Vec<_>>();

    // Create the buffers
    let instance_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some(&*(name.clone() + " Instance Buffer")),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        }
    );
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&*(name.clone() + " Vertex Buffer")),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&*(name.clone() + " Index Buffer")),
        contents: bytemuck::cast_slice(indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Crate bind group
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&texture.sampler),
            },
        ],
        label: Some(&*(name.clone() + " Bind Group")),
    });

    texture::Object {
        name,
        texture,
        bind_group,
        vertex_buffer,
        index_buffer,
        num_indices,
        instances,
        instance_buffer,
    }
}