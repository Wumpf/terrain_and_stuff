const SHADERS_DIR: &str = "shaders/";
const OUTPUT_FILE: &str = "src/shaders_embedded.rs";

fn main() {
    println!("cargo:rerun-if-changed={}", SHADERS_DIR);

    let mut shader_count = 0;
    let shader_files = walkdir::WalkDir::new(SHADERS_DIR)
        .into_iter()
        .filter_map(|entry| {
            let Ok(entry) = entry else {
                return None;
            };
            if entry.file_type().is_file() {
                let path = entry.path().to_str().unwrap().to_owned().replace('\\', "/");
                let name = path.strip_prefix(SHADERS_DIR).unwrap();
                shader_count += 1;
                Some(format!(r#"    ("{name}", include_str!("../{path}")),"#))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let contents = format!(
        r#"// This file is autogenerated via build.rs.
// DO NOT EDIT.

pub const SHADER_FILES: [(&str, &str); {shader_count}] = [
{shader_files}
];
"#,
    );

    std::fs::write(OUTPUT_FILE, contents).unwrap();
}