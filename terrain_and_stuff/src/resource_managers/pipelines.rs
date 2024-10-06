use std::{collections::HashMap, path::PathBuf};

use itertools::{self as _, Itertools};

slotmap::new_key_type! { pub struct RenderPipelineHandle; }

#[cfg(not(target_arch = "wasm32"))]
const SHADERS_DIR: &str = "terrain_and_stuff/shaders";

pub struct ShaderEntryPoint {
    /// Path relative to the `shaders` directory.
    pub path: PathBuf,
    /// The actual shader entry point.
    pub function_name: String,
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

struct RenderPipelineEntry {
    pipeline: wgpu::RenderPipeline,
    descriptor: RenderPipelineDescriptor,
}

struct ShaderModuleEntry {
    module: wgpu::ShaderModule,
    // TODO: Track dependencies of this shader in turn.
}

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Failed to load shader for path {path:?}: {err}")]
    FailedToLoadShaderSource { path: PathBuf, err: std::io::Error },

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    FileWatcherError(#[from] notify::Error),

    #[cfg(target_arch = "wasm32")]
    #[error("Failed to find shader for path {path:?} in embedded shaders.")]
    EmbeddedShaderNotFound { path: PathBuf },
}

/// Render & compute pipeline manager with simple shader reload (native only).
///
/// Shaders are embedded in the binary on the web.
pub struct PipelineManager {
    shader_modules: HashMap<PathBuf, ShaderModuleEntry>,
    render_pipelines: slotmap::SlotMap<RenderPipelineHandle, RenderPipelineEntry>,
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
                &std::path::Path::new(SHADERS_DIR),
                notify::RecursiveMode::Recursive,
            )?;

            watcher
        };

        Ok(Self {
            shader_modules: HashMap::new(),
            render_pipelines: slotmap::SlotMap::default(),
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
        let pipeline = create_wgpu_render_pipeline(&mut self.shader_modules, &descriptor, device)?;
        let handle = self.render_pipelines.insert(RenderPipelineEntry {
            pipeline,
            descriptor,
        });

        Ok(handle)
    }

    pub fn get_render_pipeline(
        &self,
        handle: RenderPipelineHandle,
    ) -> Option<&wgpu::RenderPipeline> {
        self.render_pipelines
            .get(handle)
            .map(|entry| &entry.pipeline)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn reload_changed_pipelines(&mut self, device: &wgpu::Device) {}

    #[cfg(not(target_arch = "wasm32"))]
    pub fn reload_changed_pipelines(&mut self, device: &wgpu::Device) {
        let shader_base_path = std::path::Path::new(SHADERS_DIR).canonicalize().unwrap();

        // Sometimes several change events come in at once, which is a bit annoying because of extra log.
        // Use `itertools::unique` to filter out duplicates.
        for path in self.shader_change_rx.try_iter().unique() {
            let Ok(path) = path.canonicalize() else {
                continue;
            };
            let Ok(path) = path.strip_prefix(&shader_base_path) else {
                continue;
            };

            log::info!("Reloading shader {:?}", path);

            if self.shader_modules.remove(path).is_some() {
                // Try to reload it first in isolation, so we don't get the same shader load error on every pipeline that uses it.
                match load_shader_module(device, path) {
                    Ok(shader_module) => {
                        self.shader_modules
                            .insert(path.to_path_buf(), shader_module);
                        log::info!("Successfully reloaded shader {:?}", path);
                    }
                    Err(err) => {
                        log::error!("Failed to reload shader {:?}: {:?}", path, err);
                        continue; // Keep pipelines outdated.
                    }
                }
            }

            // Try to recreate all pipelines that use this shader.
            for render_pipeline in self.render_pipelines.values_mut() {
                if &render_pipeline.descriptor.vertex_shader.path == path
                    || &render_pipeline.descriptor.fragment_shader.path == path
                {
                    match create_wgpu_render_pipeline(
                        &mut self.shader_modules,
                        &render_pipeline.descriptor,
                        device,
                    ) {
                        Ok(wgpu_pipeline) => {
                            log::info!(
                                "Recreated pipeline {:?}",
                                render_pipeline.descriptor.debug_label
                            );
                            render_pipeline.pipeline = wgpu_pipeline;
                        }
                        Err(err) => {
                            // This actually shouldn't happen since errors on pipeline creation itself are usually delayed.
                            log::error!(
                                "Failed to recreate pipeline {:?}: {:?}",
                                render_pipeline.descriptor.debug_label,
                                err
                            );
                        }
                    }
                }
            }

            // TODO: remove dependent modules.
        }
    }
}

fn create_wgpu_render_pipeline(
    shader_modules: &mut HashMap<PathBuf, ShaderModuleEntry>,
    descriptor: &RenderPipelineDescriptor,
    device: &wgpu::Device,
) -> Result<wgpu::RenderPipeline, PipelineError> {
    // Can't use `entry` here, because it doesn't allow for multiple mutable references.
    // So instead first add the shaders if they don't exist, then look them up again.
    if !shader_modules.contains_key(&descriptor.vertex_shader.path) {
        shader_modules.insert(
            descriptor.vertex_shader.path.clone(),
            load_shader_module(device, &descriptor.vertex_shader.path)?,
        );
    }
    if !shader_modules.contains_key(&descriptor.fragment_shader.path) {
        shader_modules.insert(
            descriptor.fragment_shader.path.clone(),
            load_shader_module(device, &descriptor.fragment_shader.path)?,
        );
    }
    let vertex_shader_module = &shader_modules
        .get(&descriptor.vertex_shader.path)
        .unwrap()
        .module;
    let fragment_shader_module = &shader_modules
        .get(&descriptor.fragment_shader.path)
        .unwrap()
        .module;

    let targets = descriptor
        .fragment_targets
        .iter()
        .map(|target| Some(target.clone()))
        .collect::<Vec<_>>();
    let wgpu_desc = wgpu::RenderPipelineDescriptor {
        label: Some(&descriptor.debug_label),
        layout: Some(&descriptor.layout),
        vertex: wgpu::VertexState {
            module: &vertex_shader_module,
            entry_point: Some(&descriptor.vertex_shader.function_name),
            compilation_options: shader_compilation_options(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &fragment_shader_module,
            entry_point: Some(&descriptor.fragment_shader.function_name),
            compilation_options: shader_compilation_options(),
            targets: &targets,
        }),
        primitive: descriptor.primitive,
        depth_stencil: descriptor.depth_stencil.clone(),
        multisample: descriptor.multisample,
        multiview: None,
        cache: None,
    };
    let pipeline = device.create_render_pipeline(&wgpu_desc);
    Ok(pipeline)
}

fn load_shader_module(
    device: &wgpu::Device,
    path: &std::path::Path,
) -> Result<ShaderModuleEntry, PipelineError> {
    let source;
    #[cfg(target_arch = "wasm32")]
    {
        let path_str = path.to_str().unwrap();
        source = crate::shaders_embedded::SHADER_FILES
            .iter()
            .find_map(|(name, source)| {
                if name == &path_str {
                    Some(source)
                } else {
                    None
                }
            })
            .ok_or(PipelineError::EmbeddedShaderNotFound {
                path: path.to_path_buf(),
            })?
            .to_owned();
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::path::Path::new(SHADERS_DIR).join(path);
        source = std::fs::read_to_string(&path).map_err(|err| {
            PipelineError::FailedToLoadShaderSource {
                path: path.to_path_buf(),
                err,
            }
        })?;
    };

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: path.to_str(),
        #[allow(clippy::needless_borrow)] // On Web this is a needless borrow, but on native it's not.
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&source)),
    });

    Ok(ShaderModuleEntry { module })
}

fn shader_compilation_options() -> wgpu::PipelineCompilationOptions<'static> {
    wgpu::PipelineCompilationOptions::default()
}
