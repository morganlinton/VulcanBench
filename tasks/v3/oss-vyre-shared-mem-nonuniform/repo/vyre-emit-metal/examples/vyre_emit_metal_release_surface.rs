//! Minimal release-surface example for the Metal artifact emitter.

fn main() {
    println!(
        "vyre-emit-metal artifact schema: {}",
        vyre_emit_metal::METAL_ARTIFACT_SCHEMA
    );
    println!(
        "Default MSL version: {}.{}",
        vyre_emit_metal::DEFAULT_MSL_VERSION.0,
        vyre_emit_metal::DEFAULT_MSL_VERSION.1
    );
    println!(
        "Configure artifact emission with {}.",
        std::any::type_name::<vyre_emit_metal::MetalEmitOptions>()
    );
}
