use std::{collections::HashSet, hash::Hash, path::PathBuf};

use itertools::{self as _};

use super::shader_cache::{ShaderCache, ShaderCacheError, ShaderHandle};

slotmap::new_key_type! { pub struct RenderPipelineHandle; }

#[cfg(not(target_arch = "wasm32"))]
const SHADERS_DIR: &str = "terrain_and_stuff/shaders";

pub struct ShaderEntryPoint {
    /// Path relative to the `shaders` directory.
    pub path: PathBuf,

    /// The actual shader entry point. If `None`, picks entry point with first matching type.
    pub function_name: Option<String>,
}

impl ShaderEntryPoint {
    /// First matching shader entry point in the shader file.
    pub fn first_in(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            function_name: None,
        }
    }
}

/// Render pipeline descriptor, mostly a copy of [`wgpu::RenderPipelineDescriptor`],
/// but without the lifetime dependencies & special handling for shaders.
///
/// Also, leaving out some fields  that I don't need & simplifying others.
/// (like vertex buffers. Srsly who needs vertex buffers in this time and day when you can just always do programmable pulling ;-))
pub struct RenderPipelineDescriptor {
    pub debug_label: String,
    pub layout: wgpu::PipelineLayout, // TODO: pipeline layout sharing? Add a manager? Probably not that important.
    pub vertex_shader: ShaderEntryPoint,
    pub fragment_shader: ShaderEntryPoint,
    pub fragment_targets: Vec<wgpu::ColorTargetState>,
    pub primitive: wgpu::PrimitiveState,
    pub depth_stencil: Option<wgpu::DepthStencilState>,
    pub multisample: wgpu::MultisampleState,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct RenderPipelineEntry {
    pipeline: wgpu::RenderPipeline,
    descriptor: RenderPipelineDescriptor,
    shader_handles: HashSet<ShaderHandle>,
}

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    FileWatcherError(#[from] notify::Error),

    #[error(transparent)]
    ShaderLoadError(#[from] ShaderCacheError),

    #[error("Pipeline handle is invalid.")]
    MissingPipeline,
}

/// Render & compute pipeline manager with simple shader reload (native only).
///
/// Shaders are embedded in the binary on the web.
pub struct PipelineManager {
    shader_cache: ShaderCache,
    render_pipelines: slotmap::SlotMap<RenderPipelineHandle, RenderPipelineEntry>,
    render_pipelines_with_broken_shaders: HashSet<RenderPipelineHandle>,

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    shader_change_rx: std::sync::mpsc::Receiver<PathBuf>,

    //compute_pipelines: slotmap::SlotMap<PipelineKey, wgpu::ComputePipeline>,
    #[cfg(not(target_arch = "wasm32"))]
    _filewatcher: notify::RecommendedWatcher,
}

impl PipelineManager {
    pub fn new() -> Result<Self, PipelineError> {
        let (_shader_change_tx, shader_change_rx) = std::sync::mpsc::channel();

        #[cfg(not(target_arch = "wasm32"))]
        let filewatcher = {
            let mut watcher =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                    Ok(event) => match event.kind {
                        notify::EventKind::Any
                        | notify::EventKind::Modify(notify::event::ModifyKind::Any)
                        | notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
                            for path in event.paths {
                                if let Err(err) = _shader_change_tx.send(path) {
                                    log::error!("Failed to send shader change event: {}", err);
                                }
                            }
                        }

                        notify::EventKind::Access(_)
                        | notify::EventKind::Create(_)
                        | notify::EventKind::Remove(_)
                        | notify::EventKind::Other
                        | notify::EventKind::Modify(_) => {
                            // Reloading doesn't make sense?
                        }
                    },
                    Err(err) => log::error!("Failed to watch shaders directory: {}", err),
                })?;

            notify::Watcher::watch(
                &mut watcher,
                std::path::Path::new(SHADERS_DIR),
                notify::RecursiveMode::Recursive,
            )?;

            watcher
        };

        Ok(Self {
            shader_cache: ShaderCache::new(),
            render_pipelines: slotmap::SlotMap::default(),
            render_pipelines_with_broken_shaders: HashSet::new(),
            //compute_pipelines: slotmap::SlotMap::default(),
            shader_change_rx,
            #[cfg(not(target_arch = "wasm32"))]
            _filewatcher: filewatcher,
        })
    }

    pub fn create_render_pipeline(
        &mut self,
        device: &wgpu::Device,
        descriptor: RenderPipelineDescriptor,
    ) -> Result<RenderPipelineHandle, PipelineError> {
        let (pipeline, shader_handles) =
            create_wgpu_render_pipeline(&mut self.shader_cache, &descriptor, device)?;
        let handle = self.render_pipelines.insert(RenderPipelineEntry {
            pipeline,
            descriptor,
            shader_handles,
        });

        Ok(handle)
    }

    pub fn get_render_pipeline(
        &self,
        handle: RenderPipelineHandle,
    ) -> Result<&wgpu::RenderPipeline, PipelineError> {
        self.render_pipelines
            .get(handle)
            .map(|entry| &entry.pipeline)
            .ok_or(PipelineError::MissingPipeline)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn reload_changed_pipelines(&mut self, _device: &wgpu::Device) {}

    #[cfg(not(target_arch = "wasm32"))]
    pub fn reload_changed_pipelines(&mut self, device: &wgpu::Device) {
        use itertools::Itertools as _;

        let shader_base_path = std::path::Path::new(SHADERS_DIR).canonicalize().unwrap();

        // Sometimes several change events come in at once, which is a bit annoying because of extra log.
        // Use `itertools::unique` to filter out duplicates.
        for path in self.shader_change_rx.try_iter().unique() {
            let Ok(path) = path.canonicalize() else {
                continue;
            };
            let Ok(relative_path) = path.strip_prefix(&shader_base_path) else {
                continue;
            };

            log::info!("Reloading shader {:?}", relative_path);
            let removed_shaders = self.shader_cache.remove_shader_for_path(relative_path);

            // Try to recreate all pipelines that use this shader.
            for (render_pipeline_handle, render_pipeline) in self.render_pipelines.iter_mut() {
                let affected_by_removed_shaders = removed_shaders
                    .iter()
                    .any(|removed_shader| render_pipeline.shader_handles.contains(removed_shader));
                // Any pipeline that previously failed to reload no longer points to valid shaders which is why we have to check them separately.
                let waiting_for_repaired_shader = self
                    .render_pipelines_with_broken_shaders
                    .contains(&render_pipeline_handle);

                if !affected_by_removed_shaders && !waiting_for_repaired_shader {
                    continue;
                }

                let label = &render_pipeline.descriptor.debug_label;
                log::info!("Recreating pipeline {label:?}");

                match create_wgpu_render_pipeline(
                    &mut self.shader_cache,
                    &render_pipeline.descriptor,
                    device,
                ) {
                    Ok((wgpu_pipeline, shader_handles)) => {
                        render_pipeline.pipeline = wgpu_pipeline;
                        render_pipeline.shader_handles = shader_handles;
                        self.render_pipelines_with_broken_shaders
                            .remove(&render_pipeline_handle);
                    }
                    Err(err) => {
                        log::error!("Failed to recreate pipeline {label:?}:\n{err}");
                        self.render_pipelines_with_broken_shaders
                            .insert(render_pipeline_handle);
                    }
                }
            }
        }
    }
}

fn create_wgpu_render_pipeline(
    shader_cache: &mut ShaderCache,
    descriptor: &RenderPipelineDescriptor,
    device: &wgpu::Device,
) -> Result<(wgpu::RenderPipeline, HashSet<ShaderHandle>), PipelineError> {
    let vertex_shader_handle =
        shader_cache.get_or_load_shader_module(device, &descriptor.vertex_shader.path)?;
    let fragment_shader_handle =
        shader_cache.get_or_load_shader_module(device, &descriptor.fragment_shader.path)?;

    let vertex_shader_module = shader_cache
        .shader_module(vertex_shader_handle)
        .expect("Invalid shader handle");
    let fragment_shader_module = shader_cache
        .shader_module(fragment_shader_handle)
        .expect("Invalid shader handle");

    let targets = descriptor
        .fragment_targets
        .iter()
        .map(|target| Some(target.clone()))
        .collect::<Vec<_>>();
    let wgpu_desc = wgpu::RenderPipelineDescriptor {
        label: Some(&descriptor.debug_label),
        layout: Some(&descriptor.layout),
        vertex: wgpu::VertexState {
            module: vertex_shader_module,
            entry_point: descriptor.vertex_shader.function_name.as_deref(),
            compilation_options: pipeline_compilation_options(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: fragment_shader_module,
            entry_point: descriptor.fragment_shader.function_name.as_deref(),
            compilation_options: pipeline_compilation_options(),
            targets: &targets,
        }),
        primitive: descriptor.primitive,
        depth_stencil: descriptor.depth_stencil.clone(),
        multisample: descriptor.multisample,
        multiview: None,
        cache: None,
    };
    let pipeline = device.create_render_pipeline(&wgpu_desc);

    let shader_handles = [vertex_shader_handle, fragment_shader_handle]
        .into_iter()
        .collect();

    Ok((pipeline, shader_handles))
}

fn pipeline_compilation_options() -> wgpu::PipelineCompilationOptions<'static> {
    wgpu::PipelineCompilationOptions::default()
}
