// Studio renderer of the lab: shadow map, half-resolution AO, PBR forward pass, bloom, tone map.

struct U {
  view_proj: mat4x4f,
  view: mat4x4f,
  light_vp: mat4x4f,
  eye: vec4f,
  sun: vec4f,        // direction to the light, w = intensity
  sun_color: vec4f,
  rim: vec4f,        // direction to the light, w = intensity
  rim_color: vec4f,
  sky: vec4f,        // w = softbox intensity
  ground: vec4f,
  bg_top: vec4f,
  bg_low: vec4f,
  shadow_tint: vec4f, // w = strength of the light pool on the floor
  params: vec4f,     // 1/w, 1/h, aspect, tan(fov/2)
  post: vec4f,       // exposure, bloom, 1 when the target is already sRGB, AO strength
};

@group(0) @binding(0) var<uniform> u: U;

struct VIn {
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) m0: vec4f,
  @location(3) m1: vec4f,
  @location(4) m2: vec4f,
  @location(5) color: vec4f,  // rgb, emissive
  @location(6) mat: vec4f,    // metallic, roughness, kind (1 = floor), unused
};

struct VOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) color: vec4f,
  @location(3) mat: vec4f,
};

fn world_pos(v: VIn) -> vec3f {
  let p = vec4f(v.pos, 1.0);
  return vec3f(dot(v.m0, p), dot(v.m1, p), dot(v.m2, p));
}

fn world_normal(v: VIn) -> vec3f {
  // Cofactor matrix: correct under non-uniform scale.
  let a = vec3f(v.m0.x, v.m1.x, v.m2.x);
  let b = vec3f(v.m0.y, v.m1.y, v.m2.y);
  let c = vec3f(v.m0.z, v.m1.z, v.m2.z);
  return normalize(mat3x3f(cross(b, c), cross(c, a), cross(a, b)) * v.normal);
}

@vertex fn vs_shadow(v: VIn) -> @builtin(position) vec4f {
  return u.light_vp * vec4f(world_pos(v), 1.0);
}

@vertex fn vs_main(v: VIn) -> VOut {
  var o: VOut;
  o.world = world_pos(v);
  o.clip = u.view_proj * vec4f(o.world, 1.0);
  o.normal = world_normal(v);
  o.color = v.color;
  o.mat = v.mat;
  return o;
}

// Half-resolution normals and view depth, for the AO.
@fragment fn fs_gbuffer(v: VOut) -> @location(0) vec4f {
  let n = (u.view * vec4f(normalize(v.normal), 0.0)).xyz;
  let z = (u.view * vec4f(v.world, 1.0)).z;
  return vec4f(n, z);
}

struct FullOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

@vertex fn vs_full(@builtin(vertex_index) i: u32) -> FullOut {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  var o: FullOut;
  o.clip = vec4f(p * 2.0 - 1.0, 0.9999, 1.0);
  o.uv = vec2f(p.x, 1.0 - p.y);
  return o;
}

const PI: f32 = 3.14159265;

fn ign(p: vec2f) -> f32 {
  return fract(52.9829189 * fract(dot(p, vec2f(0.06711056, 0.00583715))));
}

fn backdrop(uv: vec2f) -> vec3f {
  let c = mix(u.bg_top.rgb, u.bg_low.rgb, smoothstep(0.0, 1.0, uv.y));
  let d = (uv - vec2f(0.5, 0.62)) * vec2f(u.params.z, 1.0);
  return c * (1.0 + u.shadow_tint.w * exp(-dot(d, d) * 3.5));
}

@fragment fn fs_bg(i: FullOut) -> @location(0) vec4f {
  return vec4f(backdrop(i.uv), 1.0);
}

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_cmp: sampler_comparison;
@group(1) @binding(2) var ao_tex: texture_2d<f32>;
@group(1) @binding(3) var lin: sampler;

fn shadow(world: vec3f, n: vec3f, px: vec2f) -> f32 {
  let p = u.light_vp * vec4f(world + n * 0.035, 1.0);
  let uv = p.xy * vec2f(0.5, -0.5) + 0.5;
  if any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) {
    return 1.0;
  }
  let rot = ign(px) * 2.0 * PI;
  var s = 0.0;
  for (var i = 0; i < 12; i++) {
    let r = sqrt((f32(i) + 0.5) / 12.0) * 0.0032;
    let a = f32(i) * 2.39996 + rot;
    s += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + vec2f(cos(a), sin(a)) * r, p.z - 0.0006);
  }
  return s / 12.0;
}

// A studio made of soft boxes: what gives metal and gloss something to reflect.
fn env(d: vec3f, rough: f32) -> vec3f {
  let w = 0.04 + rough * 0.55;
  var c = mix(u.ground.rgb, u.sky.rgb, smoothstep(-0.35 - w, 0.45 + w, d.y));
  let top = smoothstep(0.72 - w, 0.72 + w, d.y);
  let a = atan2(d.x, d.z);
  let band = smoothstep(0.55 + w, 0.1, abs(d.y - 0.1));
  let left = smoothstep(0.32 + w, 0.1, abs(a + 1.15)) * band;
  let right = smoothstep(0.20 + w, 0.05, abs(a - 1.75)) * band;
  c += u.sky.rgb * u.sky.w * (top * 1.5 + left * 2.4 + right * 1.4);
  return c;
}

fn ggx(nh: f32, a: f32) -> f32 {
  let a2 = a * a;
  let d = nh * nh * (a2 - 1.0) + 1.0;
  return a2 / (PI * d * d + 1e-5);
}

fn vis(nl: f32, nv: f32, a: f32) -> f32 {
  let k = a * 0.5;
  return 0.25 / (mix(nl, 1.0, k) * mix(nv, 1.0, k) + 1e-4);
}

fn fresnel(f0: vec3f, c: f32) -> vec3f {
  return f0 + (1.0 - f0) * pow(clamp(1.0 - c, 0.0, 1.0), 5.0);
}

fn direct(n: vec3f, v: vec3f, l: vec3f, albedo: vec3f, f0: vec3f, metal: f32, a: f32) -> vec3f {
  let nl = max(dot(n, l), 0.0);
  if nl <= 0.0 {
    return vec3f(0.0);
  }
  let h = normalize(v + l);
  let nv = max(dot(n, v), 0.02);
  let spec = fresnel(f0, max(dot(v, h), 0.0)) * ggx(max(dot(n, h), 0.0), a) * vis(nl, nv, a);
  return (albedo * (1.0 - metal) / PI + spec) * nl;
}

@fragment fn fs_main(i: VOut) -> @location(0) vec4f {
  let uv = i.clip.xy * u.params.xy;
  let ao = mix(1.0, textureSampleLevel(ao_tex, lin, uv, 0.0).r, u.post.w);
  let n = normalize(i.normal);
  let sh = shadow(i.world, n, i.clip.xy);
  if i.mat.z > 0.5 {
    // The floor only catches shadow: it melts into the backdrop, with no visible edge.
    let k = mix(u.shadow_tint.rgb, vec3f(1.0), sh) * ao;
    return vec4f(backdrop(uv) * k, 1.0);
  }
  let v = normalize(u.eye.xyz - i.world);
  let metal = i.mat.x;
  let rough = clamp(i.mat.y, 0.05, 1.0);
  let a = rough * rough;
  let albedo = i.color.rgb;
  let f0 = mix(vec3f(0.04), albedo, metal);
  var c = direct(n, v, u.sun.xyz, albedo, f0, metal, a) * u.sun_color.rgb * u.sun.w * sh;
  c += direct(n, v, u.rim.xyz, albedo, f0, metal, a) * u.rim_color.rgb * u.rim.w;
  let nv = max(dot(n, v), 0.0);
  let f = f0 + (max(vec3f(1.0 - rough), f0) - f0) * pow(1.0 - nv, 5.0);
  let refl = env(reflect(-v, n), rough);
  c += albedo * (1.0 - metal) * env(n, 1.0) * (1.0 - f) * ao;
  c += refl * f * mix(1.0, ao, 0.7);
  c += albedo * i.color.a;
  return vec4f(min(c, vec3f(40.0)), 1.0);
}

// --- AO ---

@group(1) @binding(0) var g_tex: texture_2d<f32>;
@group(1) @binding(1) var ao_in: texture_2d<f32>;

fn view_pos(uv: vec2f, z: f32) -> vec3f {
  let t = u.params.w;
  return vec3f((uv.x * 2.0 - 1.0) * u.params.z * t * z, (1.0 - uv.y * 2.0) * t * z, z);
}

const AO_RADIUS: f32 = 0.9;
const AO_SAMPLES: i32 = 14;

@fragment fn fs_ao(i: FullOut) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(g_tex));
  let g = textureLoad(g_tex, vec2i(i.clip.xy), 0);
  if g.w <= 0.0 {
    return vec4f(1.0);
  }
  let p = view_pos(i.clip.xy / size, g.w);
  let px = AO_RADIUS * size.y * 0.5 / (u.params.w * g.w);
  let spin = ign(i.clip.xy) * 2.0 * PI;
  var occ = 0.0;
  for (var k = 0; k < AO_SAMPLES; k++) {
    let t = (f32(k) + 0.5) / f32(AO_SAMPLES);
    let a = spin + f32(k) * 2.39996;
    let s = i.clip.xy + vec2f(cos(a), sin(a)) * sqrt(t) * px;
    let q = textureLoad(g_tex, clamp(vec2i(s), vec2i(0), vec2i(size) - 1), 0);
    if q.w <= 0.0 {
      continue;
    }
    let d = view_pos(s / size, q.w) - p;
    let d2 = dot(d, d);
    let fall = 1.0 - smoothstep(0.0, 1.0, d2 / (AO_RADIUS * AO_RADIUS * 2.0));
    occ += max(dot(g.xyz, d) - 0.012 * g.w * 0.1, 0.0) / (d2 + 0.02) * fall;
  }
  return vec4f(clamp(1.0 - occ * 2.2 / f32(AO_SAMPLES), 0.0, 1.0));
}

@fragment fn fs_ao_blur(i: FullOut) -> @location(0) vec4f {
  let at = vec2i(i.clip.xy);
  let z = textureLoad(g_tex, at, 0).w;
  var sum = 0.0;
  var w = 0.0;
  for (var y = -2; y <= 1; y++) {
    for (var x = -2; x <= 1; x++) {
      let q = at + vec2i(x, y);
      let k = 1.0 / (0.02 + abs(textureLoad(g_tex, q, 0).w - z) * 8.0);
      sum += textureLoad(ao_in, q, 0).r * k;
      w += k;
    }
  }
  return vec4f(sum / w);
}

// --- Bloom ---

@group(1) @binding(0) var src: texture_2d<f32>;
@group(1) @binding(1) var src_s: sampler;

fn down13(uv: vec2f) -> vec3f {
  let t = 1.0 / vec2f(textureDimensions(src));
  var c = textureSampleLevel(src, src_s, uv, 0.0).rgb * 0.125;
  for (var i = 0; i < 4; i++) {
    let sx = select(-1.0, 1.0, (i & 1) == 1);
    let sy = select(-1.0, 1.0, (i & 2) == 2);
    c += textureSampleLevel(src, src_s, uv + t * vec2f(sx, sy), 0.0).rgb * 0.125;
    c += textureSampleLevel(src, src_s, uv + t * vec2f(sx, sy) * 2.0, 0.0).rgb * 0.03125;
  }
  for (var i = 0; i < 4; i++) {
    let a = f32(i) * PI * 0.5;
    c += textureSampleLevel(src, src_s, uv + t * vec2f(cos(a), sin(a)) * 2.0, 0.0).rgb * 0.0625;
  }
  return c;
}

@fragment fn fs_bright(i: FullOut) -> @location(0) vec4f {
  let c = min(down13(i.uv), vec3f(8.0));
  let b = max(c.r, max(c.g, c.b));
  return vec4f(c * smoothstep(1.05, 1.9, b), 1.0);
}

@fragment fn fs_down(i: FullOut) -> @location(0) vec4f {
  return vec4f(down13(i.uv), 1.0);
}

@fragment fn fs_up(i: FullOut) -> @location(0) vec4f {
  let t = 1.0 / vec2f(textureDimensions(src));
  var c = textureSampleLevel(src, src_s, i.uv, 0.0).rgb * 4.0;
  for (var k = 0; k < 4; k++) {
    let a = f32(k) * PI * 0.5;
    c += textureSampleLevel(src, src_s, i.uv + t * vec2f(cos(a), sin(a)), 0.0).rgb * 2.0;
    c += textureSampleLevel(src, src_s, i.uv + t * vec2f(cos(a + PI * 0.25), sin(a + PI * 0.25)) * 1.41, 0.0).rgb;
  }
  return vec4f(c / 16.0 * 0.8, 1.0);
}

// --- Tone map ---

@group(1) @binding(0) var hdr: texture_2d<f32>;
@group(1) @binding(1) var bloom: texture_2d<f32>;
@group(1) @binding(2) var comp_s: sampler;

// Khronos PBR Neutral: keeps the accent saturated.
fn neutral(c0: vec3f) -> vec3f {
  let start = 0.76;
  let desat = 0.15;
  let x = min(c0.r, min(c0.g, c0.b));
  let off = select(0.04, x - 6.25 * x * x, x < 0.08);
  let c = c0 - off;
  let peak = max(c.r, max(c.g, c.b));
  if peak < start {
    return c;
  }
  let d = 1.0 - start;
  let np = 1.0 - d * d / (peak + d - start);
  let g = 1.0 - 1.0 / (desat * (peak - np) + 1.0);
  return mix(c * np / peak, vec3f(np), g);
}

fn to_srgb(c: vec3f) -> vec3f {
  return mix(c * 12.92, 1.055 * pow(max(c, vec3f(1e-5)), vec3f(1.0 / 2.4)) - 0.055, step(vec3f(0.0031308), c));
}

@fragment fn fs_composite(i: FullOut) -> @location(0) vec4f {
  var c = textureSampleLevel(hdr, comp_s, i.uv, 0.0).rgb;
  c += textureSampleLevel(bloom, comp_s, i.uv, 0.0).rgb * u.post.y;
  c = neutral(max(c * u.post.x, vec3f(0.0)));
  if u.post.z < 0.5 {
    c = to_srgb(clamp(c, vec3f(0.0), vec3f(1.0)));
  }
  c += (ign(i.clip.xy) - 0.5) / 255.0;
  return vec4f(c, 1.0);
}
