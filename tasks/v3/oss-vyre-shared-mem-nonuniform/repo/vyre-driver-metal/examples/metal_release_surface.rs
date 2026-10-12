//! Minimal release-surface example for the native Metal backend.

fn main() {
    println!("Metal backend id: {}", vyre_driver_metal::METAL_BACKEND_ID);
    println!(
        "Acquire with vyre_driver_metal::acquire() on Apple targets when validating the native Metal backend."
    );
}
