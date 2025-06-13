use image::GenericImageView;
use anyhow::*;
use wgpu::util::DeviceExt;
use crate::minesweeper;
use crate::minesweeper::CellImage;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    // Needs to be stored as a constant as vertex_attr_array! returns a temporary value
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[derive(Debug)]
pub struct Instance {
    pub vertex_translation: [f32; 2],
    pub vertex_scale: [f32; 2],
    pub tex_cord_translation: [f32; 2],
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        let x_scale = self.vertex_scale[0];
        let y_scale = self.vertex_scale[1];
        let x_trans = self.vertex_translation[0];// / x_scale;
        let y_trans = self.vertex_translation[1];// / y_scale;
        //println!("(x_trans, y_trans): ({}, {})", x_trans, y_trans);
        //println!("{:?}", self);
        let thing = InstanceRaw {
            tex_coords: self.tex_cord_translation,
            model_matrix: [
                [x_scale, 0.0, 0.0, 0.0],
                [0.0, y_scale, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [x_trans, y_trans, 0.0, 1.0],
            ]
        };
        thing
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model_matrix: [[f32; 4]; 4],
    pub tex_coords: [f32; 2],
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }

    pub fn dummy_val() -> Instance {
        Instance { vertex_translation: [0.0; 2], vertex_scale: [1.0, 1.0], tex_cord_translation: [0.0; 2], }
    }
}

pub struct Texture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

pub struct Object {
    #[allow(unused)]
    pub name: String,
    pub texture: Texture,
    pub bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
}

impl Object {
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let unused_buffer_bytes = self.instance_buffer.size() as usize - self.instances.len() * size_of::<[f32; 16]>();
        let instance_data = self.instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let new_buffer_data = bytemuck::cast_slice(&instance_data);
        if unused_buffer_bytes >= 0 {
            queue.write_buffer(&self.instance_buffer, 0, &new_buffer_data);
        } else {
            self.instance_buffer = device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(&(self.name.clone() + " Instance Buffer")),
                    contents: &new_buffer_data,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                }
            );
        }
    }
    pub fn update_instance(&mut self, queue: &wgpu::Queue, index: usize) {
        let instance_data = [self.instances[index].to_raw()];
        let new_buffer_data = bytemuck::cast_slice(&instance_data);
        queue.write_buffer(&self.instance_buffer, (index * size_of::<InstanceRaw>()) as wgpu::BufferAddress, &new_buffer_data);
    }
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, label: &str) -> Self {
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                ..Default::default()
            }
        );

        Self {texture, view, sampler, }
    }

    pub fn from_bytes(device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8], label: Option<&str>)
                      -> Result<Self> {
        let image = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &image, label)
    }

    pub fn from_image(device: &wgpu::Device, queue: &wgpu::Queue, image: &image::DynamicImage, label: Option<&str>)
                      -> Result<Self> {
        let rgba = image.to_rgba8();
        let dimensions = image.dimensions();

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            // All textures are stored as 3D, we represent our 2D texture
            // by setting depth to 1.
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Most images are stored using sRGB, so we need to reflect that here.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
            // COPY_DST means that we want to copy data to this texture
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label,
            // This is the same as with the SurfaceConfig. It
            // specifies what texture formats can be used to
            // create TextureViews for this texture. The base
            // texture format (Rgba8UnormSrgb in this case) is
            // always supported. Note that using a different
            // texture format is not supported on the WebGL2
            // backend.
            view_formats: &[],
        });

        queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            &rgba,
            // The layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(dimensions.0 * 4),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );

        // We don't need to configure the texture view much, so let's
        // let wgpu define it.
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self { texture, view, sampler, })
    }
}

pub fn get_tex_coords(image: &CellImage) -> [f32; 2] {
    match image {
        CellImage::Zero => [0.0, 0.5],
        CellImage::One => [0.0, 0.0],
        CellImage::Two => [0.25, 0.0],
        CellImage::Three => [0.5, 0.0],
        CellImage::Four => [0.75, 0.0],
        CellImage::Five => [0.0, 0.25],
        CellImage::Six => [0.25, 0.25],
        CellImage::Seven => [0.5, 0.25],
        CellImage::Eight => [0.75, 0.25],
        CellImage::Mine => [0.5, 0.75],
        CellImage::WronglyFlagged => [0.75, 0.5],
        CellImage::SelectedMine => [0.75, 0.75],
        CellImage::Hidden => [0.25, 0.5],
        CellImage::Flagged => [0.5, 0.5],
        CellImage::QuestionMarked => [0.25, 0.75],
    }
}