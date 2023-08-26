use std::{iter, path::Path};

use cgmath::{Rad, Vector2};
use egui::{Context, FontDefinitions, Style};
use egui_winit_platform::{Platform, PlatformDescriptor};
use wgpu::{
    util::DeviceExt, LoadOp, RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    VertexBufferLayout,
};
use winit::window::Window;

use crate::{
    engine::{
        camera::{Camera, CameraUniform, Projection},
        resource::{Draw, Material, Texture},
        texture_atlas::TextureAtlas,
    },
    misc::{loader::load_string_async, Settings},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SettingsUniform {
    sunlight_intensity: u32,
    base_light_value: f32,
    light_power_factor: f32,
    tile_size: f32,
}

impl SettingsUniform {
    fn new(settings: &Settings, tile_size: f32) -> Self {
        Self {
            sunlight_intensity: settings.sunlight_intensity as u32,
            base_light_value: settings.base_light_value,
            light_power_factor: settings.light_power_factor,
            tile_size,
        }
    }

    fn update_self(&mut self, settings: &Settings) {
        self.sunlight_intensity = settings.sunlight_intensity as u32;
        self.base_light_value = settings.base_light_value;
        self.light_power_factor = settings.light_power_factor;
    }
}

pub struct Renderer<P> {
    block_material: Material,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_uniform: CameraUniform,
    projection: P,
    config: wgpu::SurfaceConfiguration,
    depth_texture: Texture,
    device: wgpu::Device,
    egui_platform: Platform,
    egui_rpass: egui_wgpu_backend::RenderPass,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    settings_bind_group: wgpu::BindGroup,
    settings_buffer: wgpu::Buffer,
    settings_uniform: SettingsUniform,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface,
    texture_atlas: TextureAtlas,
    window: Window,
}

impl<P: Projection + Sized + Default> Renderer<P> {
    pub async fn new<'a>(
        window: Window,
        vertex_desc: VertexBufferLayout<'a>,
        texture_names: &[String],
        texture_folder: &impl AsRef<Path>,
        settings: &Settings,
    ) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: {
                &[
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
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ]
            },
            label: Some("texture_bind_group_layout"),
        });

        let projection = P::default();

        let camera_uniform = CameraUniform::default();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("camera_bind_group_layout"),
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let texture_atlas = TextureAtlas::new(texture_names, texture_folder).await;
        let settings_uniform = SettingsUniform::new(settings, texture_atlas.tile_size().0);
        let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Settings Buffer"),
            contents: bytemuck::cast_slice(&[settings_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let settings_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("settings_bind_group_layout"),
        });
        let settings_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &settings_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: settings_buffer.as_entire_binding(),
            }],
            label: Some("settings_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                load_string_async("resource/shader/voxel.wgsl")
                    .await
                    .expect("Failed to load shader 'resource/shader/voxel.wgsl'")
                    .into(),
            ),
        });

        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &camera_bind_group_layout,
                &settings_bind_group_layout,
                &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
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
                }),
            ],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_desc],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let atlas_texture = texture_atlas.load_texture(&device, &queue);
        let block_material = Material {
            name: "BlockMaterial".into(),
            bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: {
                    &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&atlas_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&atlas_texture.sampler),
                        },
                    ]
                },
                label: None,
            }),
            diffuse_texture: atlas_texture,
        };

        let egui_platform = Platform::new(PlatformDescriptor {
            physical_width: window.inner_size().width,
            physical_height: window.inner_size().height,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Style::default(),
        });
        let egui_rpass = egui_wgpu_backend::RenderPass::new(&device, surface_format, 1);

        Self {
            block_material,
            camera_bind_group,
            camera_buffer,
            camera_uniform,
            config,
            depth_texture,
            device,
            egui_platform,
            egui_rpass,
            projection,
            queue,
            render_pipeline,
            settings_bind_group,
            settings_buffer,
            settings_uniform,
            size,
            surface,
            texture_atlas,
            window,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.projection
                .resize(cgmath::Vector2::new(new_size.width, new_size.height));

            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        }
    }

    pub fn set_vfov(&mut self, val: Rad<f32>) {
        self.projection
            .set_vfov(val, Vector2::new(self.size.width, self.size.height))
    }

    pub fn update(&mut self, camera: &impl Camera, settings: &Settings) {
        self.camera_uniform.update_view_proj(camera, &self.projection);
        self.settings_uniform.update_self(settings);

        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
        self.queue
            .write_buffer(&self.settings_buffer, 0, bytemuck::cast_slice(&[self.settings_uniform]));
    }

    pub fn render<'a>(
        &mut self,
        meshes: Vec<&impl Draw>,
        background_color: Option<(f32, f32, f32)>,
        ui: &mut impl GUI,
    ) -> Result<(), wgpu::SurfaceError> {
        self.egui_platform.update_time(ui.elapsed_secs());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if let Some((r, g, b)) = background_color {
                            LoadOp::Clear(wgpu::Color {
                                r: r as f64,
                                g: g as f64,
                                b: b as f64,
                                a: 1.0,
                            })
                        } else {
                            LoadOp::Load
                        },
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.render_pipeline);

            for mesh in meshes {
                mesh.draw(
                    &self.block_material,
                    &self.camera_bind_group,
                    &self.settings_bind_group,
                    &mut render_pass,
                )
            }
        }

        self.egui_platform.begin_frame();

        ui.show_ui(&self.egui_platform.context());

        let full_output = self.egui_platform.end_frame(Some(&self.window));
        let paint_jobs = self.egui_platform.context().tessellate(full_output.shapes);

        let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: self.config.width,
            physical_height: self.config.height,
            scale_factor: self.window.scale_factor() as f32,
        };
        let tdelta: egui::TexturesDelta = full_output.textures_delta;

        self.egui_rpass
            .add_textures(&self.device, &self.queue, &tdelta)
            .expect("add texture ok");
        self.egui_rpass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &screen_descriptor);
        self.egui_rpass
            .execute(&mut encoder, &view, &paint_jobs, &screen_descriptor, None)
            .unwrap();

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        self.egui_rpass.remove_textures(tdelta).expect("remove texture ok");

        Ok(())
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn texture_atlas(&self) -> &TextureAtlas {
        &self.texture_atlas
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    pub fn egui_platform_mut(&mut self) -> &mut Platform {
        &mut self.egui_platform
    }
}

pub trait GUI {
    fn elapsed_secs(&self) -> f64;
    fn show_ui(&mut self, ctx: &Context);
}
