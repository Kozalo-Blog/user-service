fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root_file = "proto/service.proto";

    let proto_root_dir: String = proto_root_file.split('/').take(1).collect();
    tonic_build::configure()
        .type_attribute(".", "#[cfg_attr(feature = \"serde\", derive(serde::Serialize, serde::Deserialize))]")
        .extern_path(".google.protobuf.Struct", "::prost_wkt_types::Struct")
        .compile(&[proto_root_file], &[proto_root_dir])?;
    Ok(())
}
