use std::path::PathBuf;

fn main() {
    // Collect every .proto file under <workspace-root>/proto/
    let proto_root = PathBuf::from("../../proto");

    let protos: Vec<PathBuf> = glob::glob(proto_root.join("**/*.proto").to_str().unwrap())
        .expect("glob failed")
        .filter_map(Result::ok)
        .collect();

    if protos.is_empty() {
        panic!("No .proto files found under {:?}", proto_root);
    }

    // Tell Cargo to re-run this script if any proto file changes
    for p in &protos {
        println!("cargo:rerun-if-changed={}", p.display());
    }

    prost_build::Config::new()
        // Derive serde traits on all generated structs/enums
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"snake_case\")]")
        // Output directory – placed into OUT_DIR (standard for build scripts)
        .compile_protos(&protos, &[proto_root])
        .expect("prost_build failed");
}
