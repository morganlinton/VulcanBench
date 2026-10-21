use serde::{Deserialize, Serialize};
use std::process::Command;

const MAX_CPUINFO_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentData {
    pub os: String,
    pub architecture: String,
    #[serde(default)]
    pub cpu_model: Option<String>,
    pub cpu_cores: usize,
    pub has_gpu: bool,
    #[serde(default)]
    pub gpu_devices: Vec<GpuDeviceInfo>,
    #[serde(default)]
    pub nvidia_driver_version: Option<String>,
    #[serde(default)]
    pub nvidia_cuda_version: Option<String>,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDeviceInfo {
    pub name: String,
    pub driver_version: String,
    pub memory_total_mib: Option<u64>,
    #[serde(default)]
    pub compute_capability_major: Option<u32>,
    #[serde(default)]
    pub compute_capability_minor: Option<u32>,
}

pub fn capture_environment() -> std::io::Result<EnvironmentData> {
    // Collect host information
    let os = std::env::consts::OS.to_string();
    let architecture = std::env::consts::ARCH.to_string();

    // Attempt to query CPU cores
    let cpu_cores = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1);

    let mut features = vec!["vyre-bench".to_string()];
    let gpu_devices = match nvidia_smi_gpu_devices() {
        Ok(devices) => devices,
        Err(error) => {
            features.push(format!(
                "gpu.nvidia_smi.devices_unavailable:{:?}",
                error.kind()
            ));
            Vec::new()
        }
    };
    let nvidia_versions = match nvidia_smi_versions() {
        Ok(versions) => versions,
        Err(error) => {
            features.push(format!(
                "gpu.nvidia_smi.versions_unavailable:{:?}",
                error.kind()
            ));
            NvidiaSmiVersions {
                driver_version: None,
                cuda_version: None,
            }
        }
    };
    let nvidia_gpu = !gpu_devices.is_empty();
    if nvidia_gpu {
        features.push("gpu.nvidia_smi".to_string());
    }
    if nvidia_versions.cuda_version.is_some() {
        features.push("gpu.nvidia_smi.cuda_version".to_string());
    }
    if gpu_devices
        .iter()
        .any(|device| device.compute_capability_major.is_some())
    {
        features.push("gpu.nvidia_smi.compute_capability".to_string());
    }
    let linked_dispatch_backends = vyre_driver::backend::registered_backends_by_precedence_slice()
        .iter()
        .filter(|backend| vyre_driver::backend::backend_dispatches(backend.id))
        .map(|backend| backend.id)
        .collect::<Vec<_>>();
    for backend in &linked_dispatch_backends {
        features.push(format!("backend.linked.{backend}"));
    }
    let mut usable_gpu_backend = false;
    for backend in linked_dispatch_backends {
        match vyre_driver::backend::acquire(backend) {
            Ok(_) if backend != "cpu-ref" => {
                usable_gpu_backend = true;
                features.push(format!("backend.usable.{backend}"));
            }
            Ok(_) => features.push(format!("backend.usable.{backend}")),
            Err(error) => features.push(format!("backend.unusable.{backend}:{error}")),
        }
    }
    let has_gpu = nvidia_gpu || usable_gpu_backend;

    Ok(EnvironmentData {
        os,
        architecture,
        cpu_model: cpu_model(),
        cpu_cores,
        has_gpu,
        nvidia_driver_version: nvidia_versions.driver_version.or_else(|| {
            gpu_devices
                .first()
                .map(|device| device.driver_version.clone())
        }),
        nvidia_cuda_version: nvidia_versions.cuda_version,
        gpu_devices,
        features,
    })
}

fn cpu_model() -> Option<String> {
    if let Ok(cpuinfo) = read_cpuinfo_bounded() {
        for line in cpuinfo.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            if key.trim() == "model name" {
                let model = value.trim();
                if !model.is_empty() {
                    return Some(model.to_string());
                }
            }
        }
    }
    let output = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let model = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if model.is_empty() {
        None
    } else {
        Some(model)
    }
}

fn read_cpuinfo_bounded() -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open("/proc/cpuinfo")?;
    let mut text = String::new();
    file.by_ref()
        .take(MAX_CPUINFO_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_CPUINFO_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "/proc/cpuinfo exceeded bounded read limit",
        ));
    }
    Ok(text)
}

fn nvidia_smi_gpu_devices() -> std::io::Result<Vec<GpuDeviceInfo>> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,driver_version,memory.total,compute_cap",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "nvidia-smi GPU provenance probe failed: {error}. Fix: repair NVIDIA driver visibility before collecting benchmark evidence."
                ),
            )
        })?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "nvidia-smi GPU provenance query exited with status {}: {}. Fix: repair NVIDIA driver visibility before collecting benchmark evidence.",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let devices: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_nvidia_smi_device)
        .collect();
    if devices.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "nvidia-smi GPU provenance query returned no parseable devices. Fix: verify `nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap --format=csv,noheader,nounits` reports at least one GPU.",
        ));
    }
    Ok(devices)
}

struct NvidiaSmiVersions {
    driver_version: Option<String>,
    cuda_version: Option<String>,
}

fn nvidia_smi_versions() -> std::io::Result<NvidiaSmiVersions> {
    let output = Command::new("nvidia-smi").output().map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "nvidia-smi version provenance probe failed: {error}. Fix: repair NVIDIA driver visibility before collecting benchmark evidence."
            ),
        )
    })?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "nvidia-smi version provenance query exited with status {}: {}. Fix: repair NVIDIA driver visibility before collecting benchmark evidence.",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let versions = NvidiaSmiVersions {
        driver_version: parse_nvidia_smi_header_value(&text, "Driver Version"),
        cuda_version: parse_nvidia_smi_header_value(&text, "CUDA Version"),
    };
    if versions.driver_version.is_none() || versions.cuda_version.is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "nvidia-smi version provenance query did not expose both Driver Version and CUDA Version. Fix: repair NVIDIA driver/runtime reporting before collecting benchmark evidence.",
        ));
    }
    Ok(versions)
}

fn parse_nvidia_smi_header_value(text: &str, label: &str) -> Option<String> {
    let (_, tail) = text.split_once(&format!("{label}:"))?;
    let value = tail.split_whitespace().next()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_nvidia_smi_device(line: &str) -> Option<GpuDeviceInfo> {
    let mut fields = line.split(',').map(str::trim);
    let name = fields.next()?.to_string();
    let driver_version = fields.next()?.to_string();
    let memory_total_mib = fields.next().and_then(|value| value.parse::<u64>().ok());
    let compute_capability = fields.next().and_then(parse_compute_capability);
    if name.is_empty() {
        return None;
    }
    Some(GpuDeviceInfo {
        name,
        driver_version,
        memory_total_mib,
        compute_capability_major: compute_capability.map(|(major, _minor)| major),
        compute_capability_minor: compute_capability.map(|(_major, minor)| minor),
    })
}

fn parse_compute_capability(value: &str) -> Option<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some((major, minor)) = value.split_once('.') {
        Some((major.trim().parse().ok()?, minor.trim().parse().ok()?))
    } else {
        Some((value.parse().ok()?, 0))
    }
}
