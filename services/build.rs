use glob::glob;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut protos: Vec<String> = Vec::new();

    for entry in glob("../protobufs/**/*.proto")? {
        let path = entry?;
        if let Some(result_path) = path.to_str() {
            protos.push(result_path.to_owned());
        }
    }

    let mut prost_build = prost_build::Config::new();
    prost_build.include_file("mod.rs");

    // // For development, it is nice to have the compiled messages for inspection
    // #[cfg(debug_assertions)]
    prost_build.out_dir("./src/protos");

    prost_build
        .compile_protos(&protos, &["../protobufs".to_string()])
        .unwrap();

    Ok(())
}
