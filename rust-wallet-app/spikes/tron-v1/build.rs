// build.rs — compile vendored proto/core/Tron.proto via prost-build.
// Pin: SHA 851575d (2026-07-14) from tronprotocol/java-tron per plan §Q2.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prost_build::Config::new()
        .compile_protos(&["proto/core/Tron.proto"], &["proto/", "/usr/include"])?;
    Ok(())
}
