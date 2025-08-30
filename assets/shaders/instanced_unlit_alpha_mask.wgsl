// instanced_unlit_alpha_mask.wgsl

struct Globals {
  view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> globals: Globals;

// Standard mesh vertex inputs used by Bevy’s mesh pipeline (position + normal + uv)
struct MeshVertex {
  @location(0) position: vec3<f32>,
  @location(1) normal:   vec3<f32>,
  @location(2) uv:       vec2<f32>,
  @builtin(instance_index) instance_index: u32,
};

struct VSOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

struct Instance {
  // Row-major 3x4 (last row implied [0,0,0,1])
  r0: vec4<f32>,
  r1: vec4<f32>,
  r2: vec4<f32>,
};

// Dynamic-size storage buffer; we index by instance_index
@group(1) @binding(0)
var<storage, read> instances: array<Instance>;

@group(2) @binding(0)
var base_color_tex: texture_2d<f32>;
@group(2) @binding(1)
var base_color_sampler: sampler;

@group(2) @binding(2)
var<uniform> material_params: vec2<f32>; // x = alpha_cutoff, y = double_sided(0/1)

fn instance_transform(i: u32) -> mat4x4<f32> {
  let m = instances[i];
  // Expand 3x4 → 4x4
  return mat4x4<f32>(
    vec4<f32>(m.r0.xyz, 0.0),
    vec4<f32>(m.r1.xyz, 0.0),
    vec4<f32>(m.r2.xyz, 0.0),
    vec4<f32>(m.r0.w,   m.r1.w,   m.r2.w,   1.0)
  );
}

@vertex
fn vs_main(in: MeshVertex) -> VSOut {
  let M = instance_transform(in.instance_index);
  let world_pos = M * vec4<f32>(in.position, 1.0);
  var out: VSOut;
  out.clip = globals.view_proj * world_pos;
  out.uv = in.uv;
  return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
  let color = textureSample(base_color_tex, base_color_sampler, in.uv);
  let alpha_cutoff = material_params.x;
  if (color.a < alpha_cutoff) {
    discard;
  }
  return vec4<f32>(color.rgb, 1.0);
}
