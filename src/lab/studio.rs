//! Studio renderer: draws into its own textures through wgpu and hands the finished image to
//! egui. All GPU objects live in `Studio`, created on the first frame of the lab and dropped
//! with it, so a closed lab holds no texture.

use super::mesh::{Meshes, Vtx};
use super::scene::{Camera, FOV, NEAR, V3, View, v3};
use bytemuck::{Pod, Zeroable};
use eframe::egui::{self, Rect};
use eframe::egui::PaintCallbackInfo;
use eframe::egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};
use eframe::wgpu;
use std::sync::{Arc, Mutex};

const FAR: f32 = 500.0;
const SHADOW_SIZE: u32 = 2048;
const SAMPLES: u32 = 4;
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const AO: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
const BLOOM_LEVELS: usize = 4;
const MAX_INSTANCES: usize = 4096;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Look {
    Light,
    Dark,
}

/// Shared with the paint callback; dropping the last reference frees every GPU object.
pub type Slot = Arc<Mutex<Option<Studio>>>;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Inst {
    pub m: [[f32; 4]; 3],
    /// rgb in linear light, emissive.
    pub color: [f32; 4],
    /// metallic, roughness, kind (1 = floor), unused.
    pub mat: [f32; 4],
}

type Mat = [[f32; 4]; 4];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: Mat,
    view: Mat,
    light_vp: Mat,
    eye: [f32; 4],
    sun: [f32; 4],
    sun_color: [f32; 4],
    rim: [f32; 4],
    rim_color: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    bg_top: [f32; 4],
    bg_low: [f32; 4],
    shadow_tint: [f32; 4],
    params: [f32; 4],
    post: [f32; 4],
}

fn v4(v: V3, w: f32) -> [f32; 4] {
    [v.x, v.y, v.z, w]
}

/// Column-major matrix from rows.
fn cols(r: [[f32; 4]; 4]) -> Mat {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in r.iter().enumerate() {
        for (j, x) in row.iter().enumerate() {
            m[j][i] = *x;
        }
    }
    m
}

fn mul(a: Mat, b: Mat) -> Mat {
    let mut m = [[0.0; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            m[c][r] = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    m
}

fn look_at(eye: V3, right: V3, up: V3, fwd: V3) -> Mat {
    cols([
        [right.x, right.y, right.z, -right.dot(eye)],
        [up.x, up.y, up.z, -up.dot(eye)],
        [fwd.x, fwd.y, fwd.z, -fwd.dot(eye)],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Sun with a fixed square of shadow map around `center`.
fn light_matrix(center: V3, dir: V3, radius: f32) -> Mat {
    let right = v3(0.0, 1.0, 0.0).cross(dir).norm();
    let up = dir.cross(right).norm();
    let d = radius * 2.0;
    let view = look_at(center + dir * d, right, up, dir * -1.0);
    let s = 1.0 / radius;
    mul(cols([[s, 0.0, 0.0, 0.0], [0.0, s, 0.0, 0.0], [0.0, 0.0, 1.0 / (d * 2.0), 0.0], [0.0, 0.0, 0.0, 1.0]]), view)
}

fn uniforms(view: &View, look: Look, size: [u32; 2], center: V3, radius: f32, srgb_target: bool) -> Uniforms {
    let aspect = size[0] as f32 / size[1].max(1) as f32;
    let tan = (FOV / 2.0).tan();
    let v = look_at(view.eye, view.right, view.up, view.fwd);
    let k = FAR / (FAR - NEAR);
    let p = cols([
        [1.0 / (tan * aspect), 0.0, 0.0, 0.0],
        [0.0, 1.0 / tan, 0.0, 0.0],
        [0.0, 0.0, k, -k * NEAR],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    let sun = v3(-0.45, 0.82, 0.42).norm();
    let params = [1.0 / size[0] as f32, 1.0 / size[1] as f32, aspect, tan];
    let mut u = Uniforms {
        view_proj: mul(p, v),
        view: v,
        light_vp: light_matrix(center, sun, radius),
        eye: v4(view.eye, 1.0),
        sun: v4(sun, 0.0),
        sun_color: [1.0, 0.97, 0.93, 0.0],
        rim: [0.0; 4],
        rim_color: [0.0; 4],
        sky: [0.0; 4],
        ground: [0.0; 4],
        bg_top: [0.0; 4],
        bg_low: [0.0; 4],
        shadow_tint: [0.0; 4],
        params,
        post: [1.0, 0.6, if srgb_target { 1.0 } else { 0.0 }, 1.0],
    };
    match look {
        Look::Light => {
            u.sun[3] = 1.5;
            u.rim = v4(v3(0.75, 0.35, -0.55).norm(), 0.35);
            u.rim_color = [1.0, 1.0, 1.0, 0.0];
            u.sky = [0.62, 0.63, 0.66, 0.22];
            u.ground = [0.46, 0.46, 0.48, 0.0];
            u.post[1] = 0.25;
            u.bg_top = [0.915, 0.92, 0.93, 0.0];
            u.bg_low = [0.80, 0.805, 0.82, 0.0];
            u.shadow_tint = [0.60, 0.61, 0.66, 0.05];
        }
        Look::Dark => {
            u.sun[3] = 3.0;
            u.rim = v4(v3(0.8, 0.3, -0.5).norm(), 3.2);
            u.rim_color = [0.62, 0.74, 1.0, 0.0];
            u.sky = [0.13, 0.15, 0.20, 2.2];
            u.ground = [0.022, 0.024, 0.03, 0.0];
            u.bg_top = [0.0035, 0.0038, 0.005, 0.0];
            u.bg_low = [0.008, 0.0088, 0.0115, 0.0];
            u.shadow_tint = [0.25, 0.25, 0.30, 4.0];
        }
    }
    u
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    hdr: wgpu::TextureView,
    g: wgpu::TextureView,
    g_depth: wgpu::TextureView,
    ao_raw: wgpu::TextureView,
    bloom: Vec<wgpu::TextureView>,
    ao_bg: wgpu::BindGroup,
    blur_bg: wgpu::BindGroup,
    main_bg: wgpu::BindGroup,
    bright_bg: wgpu::BindGroup,
    bloom_bgs: Vec<wgpu::BindGroup>,
    comp_bg: wgpu::BindGroup,
    ao: wgpu::TextureView,
    bytes: u64,
}

pub struct Studio {
    format: wgpu::TextureFormat,
    meshes_vb: wgpu::Buffer,
    meshes_ib: wgpu::Buffer,
    ranges: Vec<(u32, u32)>,
    inst_vb: wgpu::Buffer,
    ubuf: wgpu::Buffer,
    u_bg: wgpu::BindGroup,
    main_l: wgpu::BindGroupLayout,
    ao_l: wgpu::BindGroupLayout,
    tex_l: wgpu::BindGroupLayout,
    comp_l: wgpu::BindGroupLayout,
    shadow_view: wgpu::TextureView,
    cmp: wgpu::Sampler,
    lin: wgpu::Sampler,
    shadow: wgpu::RenderPipeline,
    gbuffer: wgpu::RenderPipeline,
    bg: wgpu::RenderPipeline,
    main: wgpu::RenderPipeline,
    ao: wgpu::RenderPipeline,
    ao_blur: wgpu::RenderPipeline,
    bright: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    targets: Option<Targets>,
}

fn attach(device: &wgpu::Device, label: &str, size: [u32; 2], format: wgpu::TextureFormat, samples: u32, usage: wgpu::TextureUsages) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size[0].max(1), height: size[1].max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

const RT: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT.union(wgpu::TextureUsages::TEXTURE_BINDING);
const ONLY_PASS: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT.union(wgpu::TextureUsages::TRANSIENT_ATTACHMENT);

fn tex_entry(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            multisampled: false,
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, kind: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(kind), count: None }
}

fn layout(device: &wgpu::Device, entries: &[wgpu::BindGroupLayoutEntry]) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries })
}

fn bind(device: &wgpu::Device, l: &wgpu::BindGroupLayout, res: &[wgpu::BindingResource]) -> wgpu::BindGroup {
    let entries: Vec<_> = res.iter().enumerate().map(|(i, r)| wgpu::BindGroupEntry { binding: i as u32, resource: r.clone() }).collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: l, entries: &entries })
}

impl Studio {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, meshes: &Meshes) -> Self {
        use wgpu::util::DeviceExt;
        let module = device.create_shader_module(wgpu::include_wgsl!("studio.wgsl"));
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let u_l = layout(
            device,
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        );
        let u_bg = bind(device, &u_l, &[ubuf.as_entire_binding()]);
        let main_l = layout(
            device,
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                sampler_entry(1, wgpu::SamplerBindingType::Comparison),
                tex_entry(2, true),
                sampler_entry(3, wgpu::SamplerBindingType::Filtering),
            ],
        );
        let ao_l = layout(device, &[tex_entry(0, false), tex_entry(1, false)]);
        let tex_l = layout(device, &[tex_entry(0, true), sampler_entry(1, wgpu::SamplerBindingType::Filtering)]);
        let comp_l = layout(device, &[tex_entry(0, true), tex_entry(1, true), sampler_entry(2, wgpu::SamplerBindingType::Filtering)]);
        let cmp = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let lin = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_view = attach(device, "lab shadow", [SHADOW_SIZE; 2], DEPTH, 1, RT);

        let pl = |groups: &[&wgpu::BindGroupLayout]| {
            let g: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|l| Some(*l)).collect();
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &g, immediate_size: 0 })
        };
        let solid = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vtx>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        let per_inst = wgpu::VertexBufferLayout {
            array_stride: size_of::<Inst>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4],
        };
        let depth = |write: bool, compare: wgpu::CompareFunction, bias: wgpu::DepthBiasState| wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: Default::default(),
            bias,
        };
        let pipeline = |layout: &wgpu::PipelineLayout,
                        vs: &str,
                        fs: Option<&str>,
                        target: Option<wgpu::TextureFormat>,
                        solid_mesh: bool,
                        ds: Option<wgpu::DepthStencilState>,
                        samples: u32,
                        blend: Option<wgpu::BlendState>| {
            let buffers: Vec<Option<wgpu::VertexBufferLayout>> = if solid_mesh { vec![Some(solid.clone()), Some(per_inst.clone())] } else { vec![] };
            let targets = [target.map(|format| wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some(vs), buffers: &buffers, compilation_options: Default::default() },
                primitive: wgpu::PrimitiveState {
                    cull_mode: solid_mesh.then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: ds,
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                fragment: fs.map(|fs| wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    targets: &targets,
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let less = wgpu::CompareFunction::LessEqual;
        let no_bias = wgpu::DepthBiasState::default();
        let shadow = pipeline(&pl(&[&u_l]), "vs_shadow", None, None, true, Some(depth(true, less, wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 })), 1, None);
        let gbuffer = pipeline(&pl(&[&u_l]), "vs_main", Some("fs_gbuffer"), Some(HDR), true, Some(depth(true, less, no_bias)), 1, None);
        let bg = pipeline(&pl(&[&u_l]), "vs_full", Some("fs_bg"), Some(HDR), false, Some(depth(false, wgpu::CompareFunction::Always, no_bias)), SAMPLES, None);
        let main = pipeline(&pl(&[&u_l, &main_l]), "vs_main", Some("fs_main"), Some(HDR), true, Some(depth(true, less, no_bias)), SAMPLES, None);
        let ao = pipeline(&pl(&[&u_l, &ao_l]), "vs_full", Some("fs_ao"), Some(AO), false, None, 1, None);
        let ao_blur = pipeline(&pl(&[&u_l, &ao_l]), "vs_full", Some("fs_ao_blur"), Some(AO), false, None, 1, None);
        let tex = pl(&[&u_l, &tex_l]);
        let bright = pipeline(&tex, "vs_full", Some("fs_bright"), Some(HDR), false, None, 1, None);
        let down = pipeline(&tex, "vs_full", Some("fs_down"), Some(HDR), false, None, 1, None);
        let add = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let up = pipeline(&tex, "vs_full", Some("fs_up"), Some(HDR), false, None, 1, Some(wgpu::BlendState { color: add, alpha: add }));
        let composite = pipeline(&pl(&[&u_l, &comp_l]), "vs_full", Some("fs_composite"), Some(format), false, None, 1, None);

        Studio {
            format,
            meshes_vb: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&meshes.verts),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            meshes_ib: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&meshes.index),
                usage: wgpu::BufferUsages::INDEX,
            }),
            ranges: meshes.ranges.clone(),
            inst_vb: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (MAX_INSTANCES * size_of::<Inst>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            ubuf,
            u_bg,
            main_l,
            ao_l,
            tex_l,
            comp_l,
            shadow_view,
            cmp,
            lin,
            shadow,
            gbuffer,
            bg,
            main,
            ao,
            ao_blur,
            bright,
            down,
            up,
            composite,
            targets: None,
        }
    }

    fn resize(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let half = [(size[0] / 2).max(1), (size[1] / 2).max(1)];
        let msaa = attach(device, "lab msaa", size, HDR, SAMPLES, ONLY_PASS);
        let depth = attach(device, "lab depth", size, DEPTH, SAMPLES, ONLY_PASS);
        let hdr = attach(device, "lab hdr", size, HDR, 1, RT);
        let g = attach(device, "lab g", half, HDR, 1, RT);
        let g_depth = attach(device, "lab g depth", half, DEPTH, 1, ONLY_PASS);
        let ao_raw = attach(device, "lab ao raw", half, AO, 1, RT);
        let ao = attach(device, "lab ao", half, AO, 1, RT);
        let bloom: Vec<_> = (0..BLOOM_LEVELS).map(|k| attach(device, "lab bloom", [(size[0] >> (k + 1)).max(1), (size[1] >> (k + 1)).max(1)], HDR, 1, RT)).collect();
        let tv = wgpu::BindingResource::TextureView;
        let tex = |v| bind(device, &self.tex_l, &[tv(v), wgpu::BindingResource::Sampler(&self.lin)]);
        let px = |s: [u32; 2]| s[0] as u64 * s[1] as u64;
        // What actually stays allocated when transient attachments live on the GPU tile memory.
        let bloom_px: u64 = (0..BLOOM_LEVELS).map(|k| px([size[0] >> (k + 1), size[1] >> (k + 1)])).sum();
        let bytes = px(size) * 8 + px(half) * (8 + 2) + bloom_px * 8 + px([SHADOW_SIZE; 2]) * 4;
        self.targets = Some(Targets {
            size,
            ao_bg: bind(device, &self.ao_l, &[tv(&g), tv(&g)]),
            blur_bg: bind(device, &self.ao_l, &[tv(&g), tv(&ao_raw)]),
            main_bg: bind(
                device,
                &self.main_l,
                &[tv(&self.shadow_view), wgpu::BindingResource::Sampler(&self.cmp), tv(&ao), wgpu::BindingResource::Sampler(&self.lin)],
            ),
            bright_bg: tex(&hdr),
            bloom_bgs: bloom.iter().map(&tex).collect(),
            comp_bg: bind(device, &self.comp_l, &[tv(&hdr), tv(&bloom[0]), wgpu::BindingResource::Sampler(&self.lin)]),
            msaa,
            depth,
            hdr,
            g,
            g_depth,
            ao_raw,
            ao,
            bloom,
            bytes,
        });
    }

    fn draw_meshes(&self, rp: &mut wgpu::RenderPass<'_>, draws: &[(usize, u32, u32)], skip_floor: bool) {
        rp.set_vertex_buffer(0, self.meshes_vb.slice(..));
        rp.set_vertex_buffer(1, self.inst_vb.slice(..));
        rp.set_index_buffer(self.meshes_ib.slice(..), wgpu::IndexFormat::Uint32);
        for &(mesh, start, end) in draws {
            if skip_floor && mesh == 0 {
                continue;
            }
            let (first, count) = self.ranges[mesh];
            rp.draw_indexed(first..first + count, 0, start..end);
        }
    }

    fn fullscreen(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>, pipeline: &wgpu::RenderPipeline, group: &wgpu::BindGroup) {
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        rp.set_pipeline(pipeline);
        rp.set_bind_group(0, &self.u_bg, &[]);
        rp.set_bind_group(1, group, &[]);
        rp.draw(0..3, 0..1);
    }

    fn encode(&self, encoder: &mut wgpu::CommandEncoder, draws: &[(usize, u32, u32)]) {
        let t = self.targets.as_ref().expect("targets exist after resize");
        let black = wgpu::Color::TRANSPARENT;
        let clear = wgpu::LoadOp::Clear(black);
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.shadow);
            rp.set_bind_group(0, &self.u_bg, &[]);
            self.draw_meshes(&mut rp, draws, true);
        }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.g,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: clear, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.g_depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_pipeline(&self.gbuffer);
            rp.set_bind_group(0, &self.u_bg, &[]);
            self.draw_meshes(&mut rp, draws, false);
        }
        self.fullscreen(encoder, &t.ao_raw, clear, &self.ao, &t.ao_bg);
        self.fullscreen(encoder, &t.ao, clear, &self.ao_blur, &t.blur_bg);
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.msaa,
                    depth_slice: None,
                    resolve_target: Some(&t.hdr),
                    ops: wgpu::Operations { load: clear, store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            rp.set_bind_group(0, &self.u_bg, &[]);
            rp.set_pipeline(&self.bg);
            rp.draw(0..3, 0..1);
            rp.set_pipeline(&self.main);
            rp.set_bind_group(1, &t.main_bg, &[]);
            self.draw_meshes(&mut rp, draws, false);
        }
        self.fullscreen(encoder, &t.bloom[0], clear, &self.bright, &t.bright_bg);
        for k in 1..BLOOM_LEVELS {
            self.fullscreen(encoder, &t.bloom[k], clear, &self.down, &t.bloom_bgs[k - 1]);
        }
        for k in (1..BLOOM_LEVELS).rev() {
            self.fullscreen(encoder, &t.bloom[k - 1], wgpu::LoadOp::Load, &self.up, &t.bloom_bgs[k]);
        }
    }
}

/// One frame of the scene, sent through egui's paint list.
pub struct Frame {
    pub slot: Slot,
    pub format: wgpu::TextureFormat,
    pub rect: Rect,
    pub cam: Camera,
    pub look: Look,
    /// Area covered by the shadow map.
    pub center: V3,
    pub radius: f32,
    /// Sorted by mesh.
    pub insts: Vec<(usize, Inst)>,
    pub build: fn() -> Meshes,
    pub report: Option<Arc<std::sync::atomic::AtomicU64>>,
}

impl Frame {
    pub fn shape(self) -> egui::PaintCallback {
        eframe::egui_wgpu::Callback::new_paint_callback(self.rect, self)
    }
}

impl CallbackTrait for Frame {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        _resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let mut slot = self.slot.lock().unwrap();
        let studio = slot.get_or_insert_with(|| Studio::new(device, self.format, &(self.build)()));
        let ppp = screen.pixels_per_point;
        let size = [(self.rect.width() * ppp).round().max(1.0) as u32, (self.rect.height() * ppp).round().max(1.0) as u32];
        studio.resize(device, size);
        if let (Some(r), Some(t)) = (&self.report, &studio.targets) {
            r.store(t.bytes, std::sync::atomic::Ordering::Relaxed);
        }

        let n = self.insts.len().min(MAX_INSTANCES);
        let data: Vec<Inst> = self.insts[..n].iter().map(|i| i.1).collect();
        queue.write_buffer(&studio.inst_vb, 0, bytemuck::cast_slice(&data));
        let mut draws: Vec<(usize, u32, u32)> = Vec::with_capacity(16);
        for (i, (mesh, _)) in self.insts[..n].iter().enumerate() {
            match draws.last_mut() {
                Some(d) if d.0 == *mesh => d.2 = i as u32 + 1,
                _ => draws.push((*mesh, i as u32, i as u32 + 1)),
            }
        }
        let view = self.cam.view(self.rect);
        let u = uniforms(&view, self.look, size, self.center, self.radius, self.format.is_srgb());
        queue.write_buffer(&studio.ubuf, 0, bytemuck::bytes_of(&u));
        studio.encode(encoder, &draws);
        Vec::new()
    }

    fn paint(&self, _info: PaintCallbackInfo, rp: &mut wgpu::RenderPass<'static>, _resources: &CallbackResources) {
        let slot = self.slot.lock().unwrap();
        let Some(s) = slot.as_ref().filter(|s| s.format == self.format) else { return };
        let Some(t) = &s.targets else { return };
        rp.set_pipeline(&s.composite);
        rp.set_bind_group(0, &s.u_bg, &[]);
        rp.set_bind_group(1, &t.comp_bg, &[]);
        rp.draw(0..3, 0..1);
    }
}
