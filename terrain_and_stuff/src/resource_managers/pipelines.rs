use std::collections::HashMap;

slotmap::new_key_type! { pub struct RenderPipelineHandle; }

#[cfg(not(target_arch = "wasm32"))]
const SHADERS_DIR: &str = "terrain_and_stuff/shaders";

pub struct ShaderEntryPoint {
    /// Path relative to the `shaders` directory.
    pub path: std::path::PathBuf,
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

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Failed to load shader for path {path:?}: {err}")]
    FailedToLoadShaderSource {
        path: std::path::PathBuf,
        err: std::io::Error,
    },

    #[cfg(target_arch = "wasm32")]
    #[error("Failed to find shader for path {path:?} in embedded shaders.")]
    EmbeddedShaderNotFound { path: std::path::PathBuf },
}

/// Render & compute pipeline manager with simple shader reload (native only).
///
/// Shaders are embedded in the binary on the web.
pub struct PipelineManager {
    // TODO: Each shader path needs to know the tree of shaders it depends on, so that we know which pipelines to reload.

    // split by type for borrow-checker convenience: can hold a reference to each at once.
    shader_modules_vertex: HashMap<std::path::PathBuf, wgpu::ShaderModule>,
    shader_modules_fragment: HashMap<std::path::PathBuf, wgpu::ShaderModule>,

    render_pipelines: slotmap::SlotMap<RenderPipelineHandle, RenderPipelineEntry>,
    //compute_pipelines: slotmap::SlotMap<PipelineKey, wgpu::ComputePipeline>,
}

impl PipelineManager {
    pub fn new() -> Self {
        Self {
            shader_modules_vertex: HashMap::new(),
            shader_modules_fragment: HashMap::new(),

            render_pipelines: slotmap::SlotMap::default(),
            //compute_pipelines: slotmap::SlotMap::default(),
        }
    }

    pub fn create_render_pipeline(
        &mut self,
        device: &wgpu::Device,
        descriptor: RenderPipelineDescriptor,
    ) -> Result<RenderPipelineHandle, PipelineError> {
        let vertex_shader = get_or_load_shader_module(
            &mut self.shader_modules_vertex,
            &descriptor.vertex_shader.path,
            device,
        )?;
        let fragment_shader = get_or_load_shader_module(
            &mut self.shader_modules_fragment,
            &descriptor.fragment_shader.path,
            device,
        )?;

        let targets = descriptor
            .fragment_targets
            .iter()
            .map(|target| Some(target.clone()))
            .collect::<Vec<_>>();

        let wgpu_desc = wgpu::RenderPipelineDescriptor {
            label: Some(&descriptor.debug_label),
            layout: Some(&descriptor.layout),
            vertex: wgpu::VertexState {
                module: vertex_shader,
                entry_point: Some(&descriptor.vertex_shader.function_name),
                compilation_options: shader_compilation_options(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: fragment_shader,
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

        Ok(self.render_pipelines.insert(RenderPipelineEntry {
            pipeline,
            descriptor,
        }))
    }

    pub fn get_render_pipeline(
        &self,
        handle: RenderPipelineHandle,
    ) -> Option<&wgpu::RenderPipeline> {
        self.render_pipelines
            .get(handle)
            .map(|entry| &entry.pipeline)
    }

    pub fn reload_changed_pipelines(&mut self, _device: &wgpu::Device) {
        // TODO: reload changed pipelines.
    }
}

fn get_or_load_shader_module<'a>(
    shaders_per_path: &'a mut HashMap<std::path::PathBuf, wgpu::ShaderModule>,
    path: &std::path::Path,
    device: &wgpu::Device,
) -> Result<&'a wgpu::ShaderModule, PipelineError> {
    match shaders_per_path.entry(path.to_path_buf()) {
        std::collections::hash_map::Entry::Occupied(occupied_entry) => {
            Ok(occupied_entry.into_mut())
        }
        std::collections::hash_map::Entry::Vacant(vacant_entry) => {
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
            Ok(vacant_entry.insert(module))
        }
    }
}

fn shader_compilation_options() -> wgpu::PipelineCompilationOptions<'static> {
    wgpu::PipelineCompilationOptions::default()
}
