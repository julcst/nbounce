use std::path::PathBuf;

pub fn search_files(path: &str, ext: &str) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|f| f.extension().map_or(false, |x| x == ext))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

macro_rules! include_shaders {
    ($label:expr, $( $shader:expr ),+ ) => {{
        // Concatenate all the shaders together
        let source_code = {
            let mut concatenated = String::new();
            $(
                concatenated.push_str(include_str!($shader));
            )+
            concatenated
        };

        wgpu::ShaderModuleDescriptor {
            label: Some($label),
            source: wgpu::ShaderSource::Wgsl(source_code.into()),
        }
    }};
}

pub(crate) use include_shaders;

macro_rules! create_shader_module {
    ($device:expr, $label:expr, $( $shader:expr ),+ ) => {{
        let shader_module_desc = include_shaders!($label, $( $shader ),+);

        #[cfg(debug_assertions)]
        {
            // In debug mode, we add checks or logging if needed
            $device.create_shader_module(shader_module_desc)
        }
        
        #[cfg(not(debug_assertions))]
        {
            // In release mode, we can optimize or avoid checks for performance
            $device.create_shader_module_unchecked(shader_module_desc)
        }
    }};
}

pub(crate) use create_shader_module;