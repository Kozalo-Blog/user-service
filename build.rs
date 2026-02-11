fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .extern_path(".google.protobuf.Struct", "::prost_wkt_types::Struct")
        .compile_protos(&["proto/service.proto"], &["proto"])?;

    Ok(())
}
