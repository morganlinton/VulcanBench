#![allow(missing_docs)]

#[global_allocator]
static GLOBAL: vyre_bench::probes::TrackingAllocator = vyre_bench::probes::TrackingAllocator;

fn main() -> anyhow::Result<()> {
    vyre_bench::cli::run_cli()
}
