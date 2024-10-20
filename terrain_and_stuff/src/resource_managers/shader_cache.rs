use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use slotmap::{SecondaryMap, SlotMap};

#[cfg(not(target_arch = "wasm32"))]
const SHADERS_DIR: &str = "terrain_and_stuff/shaders";

slotmap::new_key_type! { pub struct ShaderHandle; }

struct ShaderSourceEntry {
    file_path: PathBuf,
    source: String,

    /// All shaders that depend on this shader source directly.
    direct_dependents: Vec<ShaderHandle>,
}

pub struct ShaderModuleEntry {
    pub module: wgpu::ShaderModule,

    /// List of all shader paths that went into building this module.
    pub dependent_shaders: HashSet<PathBuf>,
}

pub struct ShaderCache {
    composer: naga_oil::compose::Composer,

    shader_sources: SlotMap<ShaderHandle, ShaderSourceEntry>,
    shader_modules: SecondaryMap<ShaderHandle, ShaderModuleEntry>,

    // Once preprocessor setting is supported, a single path buf would map to several shaders?
    shader_sources_per_path: HashMap<PathBuf, ShaderHandle>,
}

#[derive(thiserror::Error, Debug)]
pub enum ShaderCacheError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Failed to load shader for path {path:?}: {err}")]
    FailedToLoadShaderSource { path: PathBuf, err: std::io::Error },

    #[cfg(target_arch = "wasm32")]
    #[error("Failed to find shader for path {path:?} in embedded shaders.")]
    EmbeddedShaderNotFound { path: PathBuf },

    #[error(transparent)]
    NagaOilComposeError(#[from] naga_oil::compose::ComposerError),
}

impl ShaderCache {
    pub fn new() -> Self {
        // TODO: set composor caps.
        Self {
            composer: naga_oil::compose::Composer::default(),

            shader_sources: Default::default(),
            shader_modules: Default::default(),

            shader_sources_per_path: Default::default(),
        }
    }

    /// Eareses all memory of a given shader path.
    ///
    /// This recursively removes all shaders depending on this path as well.
    /// Path must be relative to [`SHADERS_DIR`].
    pub fn remove_shader_for_path(&mut self, path: &Path) {
        let Some(handle) = self.shader_sources_per_path.remove(path) else {
            return;
        };

        if let Some(shader_source) = self.shader_sources.remove(handle) {
            for dependency in shader_source.direct_dependents {
                if let Some(dependent_shader_source) = self.shader_sources.get(dependency) {
                    self.remove_shader_for_path(&dependent_shader_source.file_path.clone());
                }
            }
        }

        self.composer
            .remove_composable_module(path.to_str().expect("Shader path is not valid UTF-8"));
        self.shader_modules.remove(handle);
    }

    pub fn shader_module(&self, handle: ShaderHandle) -> Option<&ShaderModuleEntry> {
        self.shader_modules.get(handle)
    }

    /// Get or load a shader module for the given path.
    ///
    /// If the shader module is already loaded, it will be returned.
    /// TODO: support passing preprocessor options.
    pub fn get_or_load_shader_module(
        &mut self,
        device: &wgpu::Device,
        path: &Path,
    ) -> Result<ShaderHandle, ShaderCacheError> {
        let handle = if let Some(handle) = self.shader_sources_per_path.get(path) {
            *handle
        } else {
            self.get_or_load_shader_source(path)?
        };

        if self.shader_modules.contains_key(handle) {
            return Ok(handle);
        }

        let source = &self.shader_sources[handle];
        let path = path.to_str().expect("Shader path is not valid UTF-8");

        let module = self
            .composer
            .make_naga_module(naga_oil::compose::NagaModuleDescriptor {
                source: &source.source,
                file_path: path,
                shader_type: naga_oil::compose::ShaderType::Wgsl,
                shader_defs: HashMap::default(),
                additional_imports: &[],
            })?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(path),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module.to_owned())),
        });

        // Gather all dependent shaders.
        fn collect_dependent_shaders(
            source: &ShaderSourceEntry,
            shader_sources: &SlotMap<ShaderHandle, ShaderSourceEntry>,
            dependent_shaders: &mut HashSet<PathBuf>,
        ) {
            if dependent_shaders.insert(source.file_path.clone()) {
                for dependent_shader in &source.direct_dependents {
                    collect_dependent_shaders(
                        &shader_sources[*dependent_shader],
                        shader_sources,
                        dependent_shaders,
                    );
                }
            }
        }
        let mut dependent_shaders = HashSet::new();
        collect_dependent_shaders(&source, &self.shader_sources, &mut dependent_shaders);

        self.shader_modules.insert(
            handle,
            ShaderModuleEntry {
                module,
                dependent_shaders,
            },
        );

        Ok(handle)
    }

    /// Loads shader source into the composer and returns a handle if it wasn't already loaded.
    fn get_or_load_shader_source(&mut self, path: &Path) -> Result<ShaderHandle, ShaderCacheError> {
        if let Some(handle) = self.shader_sources_per_path.get(path) {
            assert!(self.shader_sources.contains_key(*handle));
            return Ok(*handle);
        }

        let source = raw_shader_source(path)?;

        let (module_name, required_imports, _) = naga_oil::compose::get_preprocessor_data(&source);
        let is_direct_dependency_of = required_imports
            .iter()
            .map(|import| self.get_or_load_shader_source(&Path::new(&import.import)))
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(module_name) = module_name {
            self.composer
                .add_composable_module(naga_oil::compose::ComposableModuleDescriptor {
                    source: &source,
                    file_path: &path.to_str().expect("Shader path is not valid UTF-8"),
                    language: naga_oil::compose::ShaderLanguage::Wgsl,
                    as_name: Some(module_name),
                    additional_imports: &[],
                    shader_defs: HashMap::default(),
                })?;
        }

        Ok(self.shader_sources.insert(ShaderSourceEntry {
            file_path: path.to_path_buf(),
            source,
            direct_dependents: is_direct_dependency_of,
        }))
    }
}

fn raw_shader_source(path: &std::path::Path) -> Result<String, ShaderCacheError> {
    #[cfg(target_arch = "wasm32")]
    {
        let path_str = path.to_str().unwrap();
        Ok(crate::shaders_embedded::SHADER_FILES
            .iter()
            .find_map(|(name, source)| {
                if name == &path_str {
                    Some(source)
                } else {
                    None
                }
            })
            .ok_or(ShaderCacheError::EmbeddedShaderNotFound {
                path: path.to_path_buf(),
            })?
            .to_owned())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::path::Path::new(SHADERS_DIR).join(path);
        Ok(std::fs::read_to_string(&path).map_err(|err| {
            ShaderCacheError::FailedToLoadShaderSource {
                path: path.to_path_buf(),
                err,
            }
        })?)
    }
}
