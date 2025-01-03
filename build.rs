use tonic_build;
use walkdir::WalkDir;

fn main() {
    if cfg!(feature = "agent") {
        let proto_dir = "src/proto";

        let proto_files: Vec<String> = WalkDir::new(proto_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "proto")
                    .unwrap_or(false)
            })
            .map(|entry| entry.path().to_str().unwrap().to_string())
            .collect();

        tonic_build::configure()
            .build_client(true)
            .build_server(false)
            .compile_protos(&proto_files, &[proto_dir])
            .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));

        println!("cargo:rerun-if-changed={}", proto_dir);
    }
}
