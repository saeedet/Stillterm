//! GPU ownership and presentation. The native caller owns the CAMetalLayer.
use crate::{
    CORRIDOR_SHADER,
    scene::{COLUMNS, ROWS, SHEETS, Scene},
};
use std::{ffi::c_void, time::Duration};
use stillterm_engine::EffectConfig;

pub struct Atlas<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bindings: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    sheets: wgpu::Texture,
    scene: Scene,
    atlas_size: [f32; 4],
    character_scale: f32,
    surface_misses: u32,
}
impl Renderer {
    /// # Safety
    /// `layer` must be a live CAMetalLayer on the main thread. Its owner must keep
    /// it alive until this renderer is dropped, including during resize and draw.
    pub unsafe fn new(
        layer: *mut c_void,
        size: [u32; 2],
        atlas: Atlas<'_>,
        effect: EffectConfig,
        glyphs: &[char],
        seed: u64,
        character_scale: f32,
    ) -> Result<Self, String> {
        if layer.is_null()
            || !valid_size(size)
            || atlas.width == 0
            || atlas.height == 0
            || atlas.width > 8192
            || atlas.height > 8192
            || atlas.rgba.len() != (u64::from(atlas.width) * u64::from(atlas.height) * 4) as usize
        {
            return Err("Invalid GPU surface or atlas dimensions".into());
        }
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });
        // SAFETY: caller keeps the live CAMetalLayer on the main thread for our lifetime.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
        }
        .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("No compatible Metal GPU")?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Stillterm corridor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        ))
        .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or("No GPU surface format")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size[0],
            height: size[1],
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Corridor perspective and glow"),
            source: wgpu::ShaderSource::Wgsl(CORRIDOR_SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Corridor"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sheets = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Deterministic rain sheets"),
            size: wgpu::Extent3d {
                width: COLUMNS,
                height: ROWS,
                depth_or_array_layers: SHEETS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Native glyph atlas"),
            size: wgpu::Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            atlas_texture.as_image_copy(),
            atlas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.width * 4),
                rows_per_image: Some(atlas.height),
            },
            atlas_texture.size(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph filtering"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera parameters"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Corridor resources"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &sheets.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &atlas_texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bindings,
            uniform,
            sheets,
            scene: Scene::new(effect, glyphs, seed),
            atlas_size: [
                atlas.width as f32,
                atlas.height as f32,
                atlas.tile_width as f32,
                atlas.tile_height as f32,
            ],
            character_scale,
            surface_misses: 0,
        })
    }
    pub fn resize(&mut self, size: [u32; 2]) -> Result<(), String> {
        if !valid_size(size) {
            return Err("Invalid GPU surface size".into());
        }
        if [self.config.width, self.config.height] != size {
            self.config.width = size[0];
            self.config.height = size[1];
            self.surface.configure(&self.device, &self.config);
        }
        Ok(())
    }
    fn upload(&mut self) {
        self.queue.write_texture(
            self.sheets.as_image_copy(),
            self.scene.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(COLUMNS * 4),
                rows_per_image: Some(ROWS),
            },
            self.sheets.size(),
        );
        let values = [
            self.config.width as f32,
            self.config.height as f32,
            0.0,
            0.0,
            self.atlas_size[0],
            self.atlas_size[1],
            self.atlas_size[2],
            self.atlas_size[3],
            self.character_scale,
            0.0,
            0.0,
            0.0,
        ];
        let mut bytes = [0u8; 48];
        for (destination, value) in bytes.chunks_exact_mut(4).zip(values) {
            destination.copy_from_slice(&value.to_ne_bytes());
        }
        self.queue.write_buffer(&self.uniform, 0, &bytes);
    }
    fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Corridor frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bindings, &[]);
        pass.draw(0..3, 0..1);
    }
    pub fn render(&mut self, elapsed: Duration) -> Result<(), String> {
        self.scene.advance(elapsed);
        self.upload();
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(
                error @ (wgpu::SurfaceError::Lost
                | wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Timeout),
            ) => {
                self.surface_misses += 1;
                if !matches!(error, wgpu::SurfaceError::Timeout) {
                    self.surface.configure(&self.device, &self.config);
                }
                if self.surface_misses >= 30 {
                    return Err("GPU surface remained unavailable for 30 frames".into());
                }
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
        };
        self.surface_misses = 0;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw(
            &mut encoder,
            &frame.texture.create_view(&Default::default()),
        );
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
    /// Diagnostic readback only; interactive frames never transfer pixels to the CPU.
    pub fn snapshot(&mut self) -> Result<Vec<u8>, String> {
        self.upload();
        let size = wgpu::Extent3d {
            width: self.config.width,
            height: self.config.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Diagnostic snapshot"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stride = (size.width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Diagnostic readback"),
            size: u64::from(stride) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw(&mut encoder, &texture.create_view(&Default::default()));
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        self.queue.submit(Some(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = buffer.slice(..).get_mapped_range();
        let mut pixels = Vec::with_capacity((size.width * size.height * 4) as usize);
        for row in mapped.chunks_exact(stride as usize) {
            pixels.extend_from_slice(&row[..(size.width * 4) as usize]);
        }
        if matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
}
fn valid_size(size: [u32; 2]) -> bool {
    size[0] > 0
        && size[1] > 0
        && size[0] <= 8192
        && size[1] <= 8192
        && u64::from(size[0]) * u64::from(size[1]) <= 4_194_304
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // Finish submitted work before the native owner releases its layer.
        self.device.poll(wgpu::Maintain::Wait);
    }
}
