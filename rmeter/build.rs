fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        // .out_dir("src")
        .compile_protos(&["../test_service/v1/v1.proto"], &["../test_service/v1"])?;
    Ok(())
}
