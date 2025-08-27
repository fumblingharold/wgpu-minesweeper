use anyhow::*;
use image::GenericImageView;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Stores info on how to scale each instance to fit the window as an x-scaling and a y-scaling.
pub struct Scaling {
    pub scaling: cgmath::Vector2<f32>,
}

impl Scaling {
    /// Creates a new [Scaling] for the given window size and needed aspect ratio.
    pub fn new(
        win_size: &winit::dpi::PhysicalSize<u32>,
        aspect_ratio_width: f32,
        aspect_ratio_height: f32,
    ) -> Self {
        let mut result = Self {
            scaling: cgmath::Vector2::new(0.0, 0.0),
        };
        result.rescale(win_size, aspect_ratio_height, aspect_ratio_width);
        result
    }

    /// Updates the camera based on the given window size and game aspect ratio.
    pub fn rescale(
        &mut self,
        win_size: &winit::dpi::PhysicalSize<u32>,
        aspect_ratio_width: f32,
        aspect_ratio_height: f32,
    ) {
        let width = win_size.width as f32;
        let height = win_size.height as f32;
        let window_ratio = (width * aspect_ratio_height) / (height * aspect_ratio_width);
        // If the window is too tall for the aspect ratio, scale the x to fit the window and y to
        // keep aspect ratio. Otherwise, scale the y to fit the window and x to keep the
        // aspect ratio.
        if window_ratio <= 1.0 {
            self.scaling.x = 1.0;
            self.scaling.y = window_ratio;
        } else {
            self.scaling.x = 1.0 / window_ratio;
            self.scaling.y = 1.0;
        }
    }

    /// Build a scaling matrix using the given camera.
    fn build_scaling_matrix(&self) -> [[f32; 4]; 4] {
        [
            [self.scaling.x, 0.0, 0.0, 0.0],
            [0.0, self.scaling.y, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

/// Stores info on how to scale each instance to fit the window as a 4x4 scaling matrix.
/// Uses #[repr(C)] for wgsl shader compatability.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScalingUniform {
    scaling: [[f32; 4]; 4],
}

impl ScalingUniform {
    /// Creates a new CameraUniform from the given Camera.
    pub fn new(camera: &Scaling) -> Self {
        Self {
            scaling: camera.build_scaling_matrix(),
        }
    }
}

/// A vertex from a mesh.
/// Uses #[repr(C)] for wgsl shader compatability.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    /// Attributes for buffer layout.
    /// Needs to be stored as a constant as vertex_attr_array! returns a temporary value.
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    /// Returns a buffer layout for [Vertex].
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// An instance of a texture object.
/// Uses #[repr(C)] for wgsl shader compatability.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub vertex_translation: [f32; 2],
    pub vertex_scale: [f32; 2],
    pub tex_coord_translation: [f32; 2],
    pub tex_coord_scale: [f32; 2],
}

impl Instance {
    /// Creates a new instance.
    pub fn new(
        vertex_translation: [f32; 2],
        vertex_scale: [f32; 2],
        tex_coord_translation: [f32; 2],
        tex_coord_scale: [f32; 2],
    ) -> Self {
        Self {
            vertex_translation,
            vertex_scale,
            tex_coord_translation,
            tex_coord_scale,
        }
    }

    /// Returns a buffer layout for [Instance].
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Represents a collection of [Instance]s.
pub struct TextureInstances {
    instances: Vec<Instance>,
}

impl TextureInstances {
    /// Creates a new [TextureInstance] from the given collection of [Instance]s.
    pub fn new(instances: Vec<Instance>) -> Self {
        Self { instances }
    }

    /// Provides a mutable reference to the underlying collection of [Instance]s.
    pub fn get_instances(&mut self) -> &mut Vec<Instance> {
        &mut self.instances
    }

    /// Replaces the collection of [Instance]s with the given [Instance]s.
    pub fn set_instances(&mut self, new_instances: Vec<Instance>) {
        self.instances = new_instances;
    }

    /// Provides a reference to the instance data in a form to be passed to a [TextureRenderer].
    pub fn get_data(&self) -> &[u8] {
        bytemuck::cast_slice(&self.instances)
    }

    /// Updates the texture coordinates of the instance at the given index.
    pub fn update_tex_coord_instance(&mut self, index: usize, tex_coord_translation: [f32; 2]) {
        self.instances[index].tex_coord_translation = tex_coord_translation;
    }
}

/// A texture ready to be rendered.
pub struct TextureRenderer {
    #[allow(unused)]
    name: String,
    atlas_width: u16,
    atlas_height: u16,
    render_pipeline: Arc<wgpu::RenderPipeline>,
    scaling_bind_group: Arc<wgpu::BindGroup>,
    texture_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    num_indices: u32,
    num_instances: u32,
}

impl TextureRenderer {
    /// Creates a new [TextureRenderer].
    pub fn new(
        device: &wgpu::Device,
        render_pipeline: Arc<wgpu::RenderPipeline>,
        scaling_bind_group: Arc<wgpu::BindGroup>,
        bind_group_layout: &wgpu::BindGroupLayout,
        name: String,
        texture: wgpu::Texture,
        indices: &[u16],
        instance_data: &[u8],
        vertices: &[Vertex],
    ) -> Self {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Get number of indices
        let num_indices = indices.len() as u32;

        // Create the buffers
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&*(name.clone() + " Instance Buffer")),
            contents: instance_data,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
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
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some(&*(name.clone() + " Bind Group")),
        });

        TextureRenderer {
            name,
            atlas_width: texture.width() as u16,
            atlas_height: texture.height() as u16,
            render_pipeline,
            scaling_bind_group,
            texture_bind_group,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            num_indices,
            num_instances: (instance_data.len() / size_of::<Instance>()) as u32,
        }
    }

    /// Updates the instance buffer to reflect the current state of instances.
    pub fn prepare(&mut self, instances: &[u8], device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.instance_buffer.size() as usize >= instances.len() * size_of::<u8>() {
            queue.write_buffer(&self.instance_buffer, 0, instances);
        } else {
            self.instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&(self.name.clone() + " Instance Buffer")),
                contents: instances,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        }
        self.num_instances = (instances.len() / size_of::<Instance>()) as u32;
    }

    /// Renders the instances that were previously provided to `prepare`.
    pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
        render_pass.set_bind_group(1, self.scaling_bind_group.as_ref(), &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..self.num_instances);
    }

    /// Returns the width of the [TextureRenderer]'s texture.
    pub fn atlas_width(&self) -> u16 {
        self.atlas_width
    }

    /// Returns the height of the [TextureRenderer]'s texture.
    pub fn atlas_height(&self) -> u16 {
        self.atlas_height
    }
}

/// Creates a texture using the given bytes as an image.
pub(crate) fn from_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
    label: Option<&str>,
) -> Result<wgpu::Texture> {
    let image = image::load_from_memory(bytes)?;

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
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // The actual pixel data
        &rgba,
        // The layout of the texture
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(dimensions.0 * 4),
            rows_per_image: Some(dimensions.1),
        },
        texture_size,
    );

    Ok(texture)
}
