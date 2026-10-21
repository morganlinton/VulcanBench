pub(super) struct Config {
    pub(super) backend: String,
    pub(super) only: Option<String>,
    pub(super) measured_samples: Option<usize>,
    pub(super) sample_timeout_secs: u64,
    pub(super) include_wgpu_comparison: bool,
    pub(super) reuse_existing: bool,
    pub(super) refresh_suites_only: bool,
    pub(super) workload_suite_only: bool,
}

pub(super) fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut backend = "cuda".to_string();
    let mut only = None;
    let mut measured_samples = Some(30usize);
    let mut sample_timeout_secs = 120u64;
    let mut include_wgpu_comparison = false;
    let mut reuse_existing = false;
    let mut refresh_suites_only = false;
    let mut workload_suite_only = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--backend" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Fix: --backend requires a backend id.".to_string());
                };
                if value != "cuda" && value != "wgpu" {
                    return Err(
                        "Fix: release-benchmarks only accepts `cuda` or `wgpu` backends."
                            .to_string(),
                    );
                }
                backend = value.clone();
                index += 2;
            }
            "--only" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Fix: --only requires a release workload family id.".to_string());
                };
                only = Some(value.clone());
                index += 2;
            }
            "--measured-samples" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Fix: --measured-samples requires a positive integer.".to_string());
                };
                let parsed = value.parse::<usize>().map_err(|error| {
                    format!("Fix: --measured-samples must be a positive integer: {error}")
                })?;
                if parsed == 0 {
                    return Err("Fix: --measured-samples must be greater than zero.".to_string());
                }
                if parsed < 30 {
                    return Err(
                        "Fix: release-benchmarks requires --measured-samples >= 30 for release evidence."
                            .to_string(),
                    );
                }
                measured_samples = Some(parsed);
                index += 2;
            }
            "--sample-timeout-secs" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("Fix: --sample-timeout-secs requires seconds.".to_string());
                };
                sample_timeout_secs = value.parse::<u64>().map_err(|error| {
                    format!("Fix: --sample-timeout-secs must be a positive integer: {error}")
                })?;
                if sample_timeout_secs == 0 {
                    return Err("Fix: --sample-timeout-secs must be greater than zero.".to_string());
                }
                index += 2;
            }
            "--include-wgpu-comparison" => {
                include_wgpu_comparison = true;
                index += 1;
            }
            "--reuse-existing" => {
                reuse_existing = true;
                index += 1;
            }
            "--refresh-suites-only" => {
                refresh_suites_only = true;
                index += 1;
            }
            "--workload-suite-only" => {
                workload_suite_only = true;
                index += 1;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- release-benchmarks [--backend cuda] [--only FAMILY] [--measured-samples N] [--sample-timeout-secs N] [--include-wgpu-comparison] [--reuse-existing] [--refresh-suites-only] [--workload-suite-only]\n\n\
                     Generates CUDA-first release benchmark JSON artifacts from the release workload matrix. WGPU comparison evidence is opt-in so CUDA release validation time is not spent on non-release-path backends by default. --reuse-existing validates already-written artifacts and reruns only missing or invalid cases. --refresh-suites-only rewrites suite/proof summaries from existing artifact JSON without running benchmarks. --workload-suite-only runs workload artifacts and suite summaries without auxiliary optimization artifacts."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown release-benchmarks option `{other}`.")),
        }
    }
    Ok(Config {
        backend,
        only,
        measured_samples,
        sample_timeout_secs,
        include_wgpu_comparison,
        reuse_existing,
        refresh_suites_only,
        workload_suite_only,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        std::iter::once("cargo".to_string())
            .chain(std::iter::once("release-benchmarks".to_string()))
            .chain(extra.iter().map(|arg| arg.to_string()))
            .collect()
    }

    #[test]
    fn refresh_suites_only_parses_without_forcing_benchmark_reuse() {
        let config = parse_args(&args(&[
            "--backend",
            "cuda",
            "--include-wgpu-comparison",
            "--refresh-suites-only",
        ]))
        .expect("Fix: release-benchmarks refresh-only args should parse.");

        assert_eq!(config.backend, "cuda");
        assert!(config.include_wgpu_comparison);
        assert!(config.refresh_suites_only);
        assert!(
            !config.reuse_existing,
            "Fix: suite summary refresh must be distinct from freshness-based benchmark reuse."
        );
    }

    #[test]
    fn workload_suite_only_parses_as_auxiliary_skip() {
        let config = parse_args(&args(&["--backend", "wgpu", "--workload-suite-only"]))
            .expect("Fix: release-benchmarks workload-suite args should parse.");

        assert_eq!(config.backend, "wgpu");
        assert!(config.workload_suite_only);
        assert!(
            !config.refresh_suites_only,
            "Fix: workload-suite execution must still run benchmark artifacts unless refresh-only is also explicit."
        );
    }
}
