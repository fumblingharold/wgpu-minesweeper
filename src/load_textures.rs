use wgpu::util::DeviceExt;
use crate::texture;

const GRID_VERTICES: &[texture::Vertex] = &[
    texture::Vertex { position: [0.0, 0.0, 0.0], tex_coords: [0.0 , 0.25], }, // A
    texture::Vertex { position: [0.0, 1.0, 0.0], tex_coords: [0.0 , 0.0 ], }, // B
    texture::Vertex { position: [1.0, 0.0, 0.0], tex_coords: [0.25, 0.25], }, // C
    texture::Vertex { position: [1.0, 1.0, 0.0], tex_coords: [0.25, 0.0 ], }, // D
];

const GRID_INDICES: &[u16] = &[
    0, 2, 1,
    1, 2, 3,
];

pub fn get_grid_texture(device: &wgpu::Device, queue: &wgpu::Queue,
                        bind_group_layout: &wgpu::BindGroupLayout, width: u32, height: u32) -> texture::Object {

    let diffuse_bytes = include_bytes!("Grid.png");
    let texture = texture::Texture::from_bytes(&device, &queue, diffuse_bytes, Some("Grid Texture"))
        .expect("Failed to load Grid Texture");

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Grid Vertex Buffer"),
        contents: bytemuck::cast_slice(GRID_VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Grid Index Buffer"),
        contents: bytemuck::cast_slice(GRID_INDICES),
        usage: wgpu::BufferUsages::INDEX,
    });

    let num_indices = GRID_INDICES.len() as u32;

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
        label: Some("Grid Bind Group"),
    });

    let half_width = width as f32 / 2.0;
    let half_height = height as f32 / 2.0;

    let instances = (0..height).flat_map(|row| {
        (0..width).map(move |col| {
            texture::Instance {
                vertex_translation: [col as f32 / half_width - 1.0, row as f32 / half_height - 1.0],
                vertex_scale: [1.0 / half_width, 1.0 / half_height],
                tex_cord_translation: [0.25, 0.5],
            }
        })
    }).collect::<Vec<_>>();

    let mut instance_data = instances.iter().map(texture::Instance::to_raw).collect::<Vec<_>>();
    //instance_data.reverse();

    let instance_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Grid Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        }
    );

    texture::Object { name: "Grid".parse().unwrap(), texture, bind_group, vertex_buffer, index_buffer, num_indices, instances, instance_buffer, }
}