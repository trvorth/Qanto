fn main() {
    // Use vendored protoc to avoid requiring a system install
    let protoc_path = protoc_bin_vendored::protoc_bin_path().expect("Vendored protoc not found");
    std::env::set_var("PROTOC", protoc_path);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["proto/qanto.proto"], &["proto"])
        .expect("Failed to compile gRPC proto");
}
