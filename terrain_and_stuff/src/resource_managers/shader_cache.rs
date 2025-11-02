use std::{collections::HashMap, hash::Hash, path::Path};

use slotmap::SlotMap;

slotmap::new_key_type! { pub struct ShaderHandle; }

struct ShaderEntry {
    module_path: wesl::ModulePath,
    feature_flags: Vec<String>,

    module: wgpu::ShaderModule,
    all_dependencies: Vec<wesl::ModulePath>,
}

#[cfg(target_arch = "wasm32")]
type WeslResolver = wesl::VirtualResolver<'static>;

#[cfg(not(target_arch = "wasm32"))]
type WeslResolver = wesl::StandardResolver;

pub struct ShaderCache {
    wesl_resolver: WeslResolver,

    shaders: SlotMap<ShaderHandle, ShaderEntry>,
    shader_per_module_per_flags: HashMap<wesl::ModulePath, HashMap<Vec<String>, ShaderHandle>>,
}

#[derive(thiserror::Error, Debug)]
pub enum ShaderCacheError {
    #[error("Wesl compiler error: {0}")]
    WeslCompilerError(#[from] wesl::Error),
}

impl ShaderCache {
    pub fn new() -> Self {
        let wesl_resolver = {
            #[cfg(target_arch = "wasm32")]
            {
                let mut resolver = wesl::VirtualResolver::new();
                for (path, content) in crate::shaders_embedded::SHADER_FILES {
                    resolver.add_module(
                        path_or_name_to_module_path(path),
                        std::borrow::Cow::Borrowed(content),
                    );
                }
                resolver
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                wesl::StandardResolver::new("terrain_and_stuff/shaders")
            }
        };

        Self {
            wesl_resolver,
            shaders: Default::default(),
            shader_per_module_per_flags: Default::default(),
        }
    }

    pub fn shader_module(&self, handle: ShaderHandle) -> Option<&wgpu::ShaderModule> {
        self.shaders.get(handle).map(|entry| &entry.module)
    }

    /// Get or load a shader module for the given path.
    ///
    /// If the shader module is already loaded, it will be returned.
    /// TODO: support passing preprocessor options.
    pub fn get_or_load_shader_module(
        &mut self,
        device: &wgpu::Device,
        module_name: &str,
        enabled_feature_flags: &[&'static str],
    ) -> Result<ShaderHandle, ShaderCacheError> {
        let module_path = path_or_name_to_module_path(module_name);
        let feature_flags: Vec<String> = enabled_feature_flags
            .iter()
            .map(|&flag| flag.to_string())
            .collect();

        if let Some(handle) = self
            .shader_per_module_per_flags
            .get(&module_path)
            .and_then(|map| map.get(&feature_flags))
        {
            log::debug!("Shader {module_name:?} already loaded with {feature_flags:?} flags");
            return Ok(*handle);
        }

        let shader = compile_shader(
            &self.wesl_resolver,
            device,
            module_path.clone(),
            feature_flags.clone(),
        )?;
        let handle = self.shaders.insert(shader);
        self.shader_per_module_per_flags
            .entry(module_path)
            .or_default()
            .insert(feature_flags, handle);

        Ok(handle)
    }

    /// Reloads all shaders that depend on a given path and returns which shaders have been reloaded
    #[cfg_attr(target_arch = "wasm32", expect(dead_code))]
    pub fn reload_all_shaders_depending_on(
        &mut self,
        device: &wgpu::Device,
        changed_file: &Path,
    ) -> Result<Vec<ShaderHandle>, ShaderCacheError> {
        let mut reloaded_shaders = Vec::new();

        let changed_module_path = path_or_name_to_module_path(changed_file.to_str().unwrap());

        for (handle, shader_entry) in self.shaders.iter_mut() {
            if shader_entry
                .all_dependencies
                .iter()
                .any(|dep| dep == &changed_module_path)
            {
                log::info!("Reloading shader {handle:?}");

                *shader_entry = compile_shader(
                    &self.wesl_resolver,
                    device,
                    shader_entry.module_path.clone(),
                    shader_entry.feature_flags.clone(),
                )?;
                reloaded_shaders.push(handle);
            }
        }

        Ok(reloaded_shaders)
    }
}

fn compile_shader(
    wesl_resolver: &WeslResolver,
    device: &wgpu::Device,
    module_path: wesl::ModulePath,
    feature_flags: Vec<String>,
) -> Result<ShaderEntry, ShaderCacheError> {
    let mut compiler = wesl::Wesl::new_barebones().set_custom_resolver(wesl_resolver);
    compiler.set_mangler(wesl::ManglerKind::Escape);
    compiler.set_options(wesl::CompileOptions {
        features: wesl::Features {
            default: wesl::Feature::Disable,
            flags: feature_flags
                .iter()
                .map(|flag| (flag.clone(), wesl::Feature::Enable))
                .collect(),
        },
        ..Default::default()
    });

    let compile_result = compiler.compile(&module_path)?;

    let wgsl_source = compile_result.to_string();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&module_path.to_string()),
        source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
    });

    Ok(ShaderEntry {
        module_path,
        feature_flags,
        module,
        all_dependencies: compile_result.modules,
    })
}

fn path_or_name_to_module_path(module_name_or_path: &str) -> wesl::ModulePath {
    let parts = module_name_or_path
        .trim_start_matches("/")
        .trim_end_matches(".wgsl")
        .trim_end_matches(".wesl")
        .replace("\\", "/")
        .split('/')
        .map(|s| s.to_owned())
        .collect();
    wesl::ModulePath::new(wesl::syntax::PathOrigin::Absolute, parts)
}
