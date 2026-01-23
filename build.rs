fn main() {
    // OTLP proto compilation will be added in Phase 2
    // when we implement the gRPC ingest endpoint

    // For now, just ensure the build script exists
    // so Cargo.toml's build-dependencies don't cause issues

    println!("cargo:rerun-if-changed=proto/");
}
