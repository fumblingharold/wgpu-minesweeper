use wgpu::util::DeviceExt;
use crate::{minesweeper, texture, seven_segment};

/// Hard coded information about the number of pixels in the textures.
const FRAME_WIDTHS: [u8; 2] = [12, 8];
const FRAME_HEIGHTS: [u8; 4] = [8, 11, 33, 12];
const GRID_LENGTH_F64: f64 = 16.0;
const GRID_LENGTH_F32: f32 = 16.0;
const GRID_LENGTH_U16: u16 = 16;

/// Returns the width of the minesweeper game in pixels given the grid's width.
pub fn get_total_pixel_width(width: minesweeper::Dim) -> u16 {
    width as u16 * GRID_LENGTH_U16 + FRAME_WIDTHS.iter().sum::<u8>() as u16
}

/// Returns the height of the minesweeper game in pixels given the grid's height.
pub fn get_total_pixel_height(height: minesweeper::Dim) -> u16 {
    height as u16 * GRID_LENGTH_U16 + FRAME_HEIGHTS.iter().sum::<u8>() as u16
}

/// Rescaled and translates a position on the image to be relative to the grid.
///
/// Should change implementation so not needed.
pub fn convert_to_over_grid(width: minesweeper::Dim, height: minesweeper::Dim, pos: cgmath::Vector2<f64>) -> cgmath::Vector2<f64> {
    let pwidth = get_total_pixel_width(width) as f64;
    let pheight = get_total_pixel_height(height) as f64;
    cgmath::Vector2::new((pos.x * pwidth - FRAME_WIDTHS[0] as f64) / width as f64 / GRID_LENGTH_F64,
                         (pos.y * pheight - FRAME_HEIGHTS[1..4].iter().sum::<u8>() as f64) / height as f64 / GRID_LENGTH_F64)
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
    let half_total_pixel_width = get_total_pixel_width(width) as f32 / 2.0;
    let half_total_pixel_height = get_total_pixel_height(height) as f32 / 2.0;
    let tex_cord_translation = texture::get_cell_tex_coords(&minesweeper::CellImage::Hidden);
    let instances = (0..height).flat_map(|row| {
        (0..width).map(move |col| {
            texture::Instance {
                vertex_translation: [
                    (FRAME_WIDTHS[0] as f32 + col as f32 * GRID_LENGTH_F32) / half_total_pixel_width - 1.0,
                    (FRAME_HEIGHTS[0] as f32 + row as f32 * GRID_LENGTH_F32) / half_total_pixel_height - 1.0],
                vertex_scale: [GRID_LENGTH_F32 / half_total_pixel_width, GRID_LENGTH_F32 / half_total_pixel_height],
                tex_cord_translation,
            }
        })
    }).collect::<Vec<_>>();

    build_texture(device, queue, bind_group_layout, "Grid".parse().unwrap(), diffuse_bytes, GRID_INDICES, instances, GRID_VERTICES)
}

/// Creates a [texture::Object] for the minesweeper border.
pub fn get_border_texture(device: &wgpu::Device, queue: &wgpu::Queue,
                          bind_group_layout: &wgpu::BindGroupLayout,
                          width: minesweeper::Dim, height: minesweeper::Dim) -> texture::Object {
    // Load grid textures into memory and create a Texture from it
    let image_data = include_bytes!("Frame.png");

    // Create index data
    let mut frame_indices: Vec<u16> = Vec::with_capacity(30);
    for x in 0..=FRAME_WIDTHS.len() as u16 {
        for y in 0..=FRAME_HEIGHTS.len() as u16 {
            let top_lft = x + 4 * y;
            let top_rgt = top_lft + 1;
            let bot_lft = top_lft + 4;
            let bot_rgt = bot_lft + 1;
            frame_indices.append(&mut vec!(bot_lft, top_rgt, top_lft, bot_rgt, top_rgt, bot_lft));
        }
    }
    let frame_indices= frame_indices.as_slice();

    // Create instance data
    let instances = vec!(texture::Instance {
        vertex_translation: [0.0, 0.0],
        vertex_scale: [1.0, 1.0],
        tex_cord_translation: [0.0, 0.0],
    });

    // Create texture data
    let width = width as f32 * 16.0;
    let height = height as f32 * 16.0;
    let total_width = width + 20.0;
    let total_height = height + 64.0;
    // Hardcoded based on texture atlas
    let tx = vec!(0.0, 12.0 / 21.0, 13.0 / 21.0, 1.0);
    let mut ty = vec!(1.0, 25.0 / 33.0, 24.0 / 33.0, 13.0 / 33.0, 12.0 / 33.0, 0.0);
    ty.reverse();
    let sx: Vec<_> = vec!(0.0, 12.0 / total_width, (width + 12.0) / total_width, 1.0).iter().map(|x| 2.0 * x - 1.0).collect();
    let mut sy = vec!(0.0, 8.0 / total_height, (height + 8.0) / total_height, (height + 19.0) / total_height, (height + 52.0) / total_height, 1.0);
    sy.reverse();
    let sy: Vec<_> = sy.iter().map(|y| 2.0 * y - 1.0).collect();
    let mut frame_vertices = Vec::with_capacity(tx.len() * ty.len());
    for (ty, sy) in ty.iter().zip(sy.iter()) {
        for (tx, sx) in tx.iter().zip(sx.iter()) {
            frame_vertices.push(texture::Vertex { position: [*sx, *sy, 0.0], tex_coords: [*tx, *ty] });
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
    let width = width as f32 * 16.0;
    let height = height as f32 * 16.0;
    let total_width = width + 20.0;
    let total_height = height + 64.0;
    let left_top_corners = [17.0, total_width - 15.0 - 13.0 * 3.0];
    let mut instances = Vec::with_capacity(6);
    let mut digits = seven_segment::get_texture_coords(mines as i32)
        .into_iter()
        .chain([[0.0, 1.0 / 12.0]; 2].into_iter())
        .chain([[0.0, 11.0 / 12.0]; 1].into_iter());
    for top_left_corner in left_top_corners.iter() {
        for digit in 0..3 {
            instances.push(texture::Instance {
                vertex_translation: [(top_left_corner + 13.0 * digit as f32) / total_width * 2.0 - 1.0, -40.0 / total_height * 2.0 + 1.0],
                vertex_scale: [13.0 / total_width * 2.0, 23.0 / total_height * 2.0],
                tex_cord_translation: digits.next().unwrap(),
            });
        }
    }

    // Create texture data
    // Hardcoded based on texture atlas
    let tx = vec!(0.0, 1.0);
    let mut ty = vec!(1.0, 253.0 / 276.0);
    ty.reverse();
    let sx = vec!(0.0, 1.0);
    let mut sy = vec!(0.0, 1.0);
    sy.reverse();
    let mut frame_vertices = Vec::with_capacity(tx.len() * ty.len());
    for (ty, sy) in ty.iter().zip(sy.iter()) {
        for (tx, sx) in tx.iter().zip(sx.iter()) {
            frame_vertices.push(texture::Vertex { position: [*sx, *sy, 0.0], tex_coords: [*tx, *ty] });
        }
    }
    let vertices = [
        texture::Vertex { position: [0.0, 0.0, 0.0], tex_coords: [0.0, 1.0 / 12.0], },
        texture::Vertex { position: [0.0, 1.0, 0.0], tex_coords: [0.0, 0.0 ], },
        texture::Vertex { position: [1.0, 0.0, 0.0], tex_coords: [1.0, 1.0 / 12.0], },
        texture::Vertex { position: [1.0, 1.0, 0.0], tex_coords: [1.0, 0.0 ], },
    ];

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