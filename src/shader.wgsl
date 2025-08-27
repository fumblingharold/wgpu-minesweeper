// Vertex Shader

struct Scaling {
    view_proj: mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> scaling: Scaling;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct InstanceInput {
    @location(5) vertex_translation: vec2<f32>,
    @location(6) vertex_scale: vec2<f32>,
    @location(7) tex_cord_translation: vec2<f32>,
    @location(8) tex_cord_scale: vec2<f32>,
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        vec4<f32>(instance.vertex_scale.x, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, instance.vertex_scale.y, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(instance.vertex_translation.x, instance.vertex_translation.y, 0.0, 1.0),
    );
    let tex_matrix = mat4x4<f32>(
        vec4<f32>(instance.tex_cord_scale.x, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, instance.tex_cord_scale.y, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(instance.tex_cord_translation.x, instance.tex_cord_translation.y, 0.0, 1.0),
    );

    var out: VertexOutput;
    out.clip_position = scaling.view_proj * model_matrix * vec4<f32>(model.position, 0.0, 1.0);
    out.tex_coords = (tex_matrix * vec4<f32>(model.tex_coords, 0.0, 1.0)).xy;
    return out;
}

// Fragment Shader

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}