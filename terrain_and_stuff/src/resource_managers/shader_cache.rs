use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
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
    direct_dependents: HashSet<ShaderHandle>,
}

pub struct ShaderCache {
    composer: naga_oil::compose::Composer,

    shader_sources: SlotMap<ShaderHandle, ShaderSourceEntry>,
    shader_modules: SecondaryMap<ShaderHandle, wgpu::ShaderModule>,

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

    #[error("Named modules are not supported, used by {path:?}")]
    NamedModuleNotSupported { path: PathBuf },

    #[error("Failed NagaOil composing step for {path:?}: {err_formatted}")]
    NagaOilComposeError {
        path: PathBuf,
        err_formatted: String,
    },
}

impl ShaderCache {
    pub fn new() -> Self {
        Self {
            composer: naga_oil::compose::Composer::default()
                // TODO: set composor caps.
                .with_capabilities(wgpu::naga::valid::Capabilities::all()),

            shader_sources: Default::default(),
            shader_modules: Default::default(),

            shader_sources_per_path: Default::default(),
        }
    }

    /// Eareses all memory of a given shader path.
    ///
    /// This recursively removes all shaders depending on this path as well.
    /// Path must be relative to [`SHADERS_DIR`].
    /// Returns a list of all shaders that were removed.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn remove_shader_for_path(&mut self, path: &Path) -> Vec<ShaderHandle> {
        let Ok(path) = resolve_path(path) else {
            log::error!("Failed to resolve shader path {path:?}");
            return vec![];
        };
        let Some(handle) = self.shader_sources_per_path.remove(&path) else {
            log::debug!("Shader for path {path:?} not found");
            return vec![];
        };

        let mut removed_shaders = vec![handle];

        if let Some(shader_source) = self.shader_sources.remove(handle) {
            for child in shader_source.direct_dependents {
                if let Some(child_shader) = self.shader_sources.get(child) {
                    removed_shaders
                        .extend(self.remove_shader_for_path(&child_shader.file_path.clone()));
                }
            }
        }

        let composer_path = composer_path(&path);
        self.composer.remove_composable_module(&composer_path);

        self.shader_modules.remove(handle);

        log::debug!("Removed shader for path {path:?}");

        removed_shaders
    }

    pub fn shader_module(&self, handle: ShaderHandle) -> Option<&wgpu::ShaderModule> {
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
        let path = resolve_path(path)?;
        let handle = if let Some(handle) = self.shader_sources_per_path.get(&path) {
            log::debug!("Shader for path {path:?} already loaded");
            *handle
        } else {
            log::debug!("Loading shader for path {path:?}");
            self.get_or_load_shader_source(&path)?
        };

        if self.shader_modules.contains_key(handle) {
            log::debug!("shader module for path {path:?} already loaded");
            return Ok(handle);
        }

        let source = &self.shader_sources[handle];
        let path_string = path.to_str().expect("Shader path is not valid UTF-8");

        let module = self
            .composer
            .make_naga_module(naga_oil::compose::NagaModuleDescriptor {
                source: &source.source,
                file_path: path_string,
                shader_type: naga_oil::compose::ShaderType::Wgsl,
                shader_defs: HashMap::default(),
                additional_imports: &[],
            })
            .map_err(|err| ShaderCacheError::NagaOilComposeError {
                path: path.to_path_buf(),
                err_formatted: err.emit_to_string(&self.composer),
            })?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(path_string),
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module.to_owned())),
        });

        self.shader_modules.insert(handle, module);

        Ok(handle)
    }

    /// Loads shader source into the composer and returns a handle if it wasn't already loaded.
    ///
    /// Path must be relative to [`SHADERS_DIR`].
    fn get_or_load_shader_source(&mut self, path: &Path) -> Result<ShaderHandle, ShaderCacheError> {
        let path = resolve_path(path)?;
        if let Some(handle) = self.shader_sources_per_path.get(&path) {
            assert!(self.shader_sources.contains_key(*handle));
            return Ok(*handle);
        }

        let source = raw_shader_source(&path)?;

        let (module_name, required_imports, shader_defs) =
            naga_oil::compose::get_preprocessor_data(&source);
        if module_name.is_some() {
            return Err(ShaderCacheError::NamedModuleNotSupported {
                path: path.to_path_buf(),
            });
        }

        let parent_shaders = required_imports
            .iter()
            .map(|import| {
                let import_path = import
                    .import
                    .trim_start_matches("\"")
                    .trim_end_matches("\"");
                self.get_or_load_shader_source(Path::new(import_path))
            })
            .collect::<Result<Vec<_>, _>>()?;

        {
            let composer_path = composer_path(&path);
            if let Err(err) =
                self.composer
                    .add_composable_module(naga_oil::compose::ComposableModuleDescriptor {
                        source: &source,
                        file_path: &composer_path,
                        language: naga_oil::compose::ShaderLanguage::Wgsl,
                        as_name: Some(format!("{composer_path:?}")),
                        additional_imports: &[],
                        shader_defs,
                    })
            {
                // Make sure composer no longer knows this module - looks like depending on the error it may still :/
                self.composer.remove_composable_module(&composer_path);

                // Can't do map_err because otherwise borrow checker gets angry.
                return Err(ShaderCacheError::NagaOilComposeError {
                    path: composer_path.into(),
                    err_formatted: err.emit_to_string(&self.composer),
                });
            }
        }

        let handle = self.shader_sources.insert(ShaderSourceEntry {
            file_path: path.to_path_buf(),
            source,
            direct_dependents: HashSet::new(),
        });
        self.shader_sources_per_path
            .insert(path.to_path_buf(), handle);

        for parent_shader in parent_shaders {
            self.shader_sources[parent_shader]
                .direct_dependents
                .insert(handle);
        }

        Ok(handle)
    }
}

fn raw_shader_source(full_path: &std::path::Path) -> Result<String, ShaderCacheError> {
    #[cfg(target_arch = "wasm32")]
    {
        let path_str = full_path.to_str().expect("Shader path is not valid UTF-8");
        Ok(crate::shaders_embedded::SHADER_FILES
            .iter()
            .find_map(|(name, source)| {
                if name == &path_str {
                    Some(*source)
                } else {
                    None
                }
            })
            .ok_or(ShaderCacheError::EmbeddedShaderNotFound {
                path: full_path.to_path_buf(),
            })?
            .to_owned())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(&full_path).map_err(|err| {
            ShaderCacheError::FailedToLoadShaderSource {
                path: full_path.to_path_buf(),
                err,
            }
        })
    }
}

#[cfg(target_arch = "wasm32")]
fn resolve_path(path: &Path) -> Result<PathBuf, ShaderCacheError> {
    Ok(path.to_path_buf())
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_path(path: &Path) -> Result<PathBuf, ShaderCacheError> {
    std::path::Path::new(SHADERS_DIR)
        .join(path)
        .canonicalize()
        .map_err(|err| ShaderCacheError::FailedToLoadShaderSource {
            path: path.into(),
            err,
        })
}

#[cfg(target_arch = "wasm32")]
fn composer_path(path: &Path) -> String {
    path.to_str()
        .expect("Shader path is not valid UTF-8")
        .to_owned()
}

#[cfg(not(target_arch = "wasm32"))]
fn composer_path(path: &Path) -> String {
    let base_path = std::path::Path::new(SHADERS_DIR).canonicalize().unwrap();
    let relative_path = path.strip_prefix(&base_path).unwrap();
    relative_path
        .to_str()
        .expect("Shader path is not valid UTF-8")
        .replace("\\", "/")
        .to_owned()
}
