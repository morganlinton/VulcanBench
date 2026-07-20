use super::*;
pub(crate) fn shared_dispatch_backend() -> Result<Arc<dyn VyreBackend>, String> {
    static BACKEND: OnceLock<Arc<dyn VyreBackend>> = OnceLock::new();
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }
    let requested = std::env::var("VYRE_BACKEND").ok();
    let backend = if let Some(id) = requested.as_deref() {
        if id == "preferred" {
            return Err(
                "C frontend dispatch backend override VYRE_BACKEND=preferred is recursive. Fix: unset VYRE_BACKEND for CUDA-first selection or set it to `cuda`/`wgpu` explicitly."
                    .to_string(),
            );
        }
        dispatch_backend_by_id(id).map_err(|error| {
            format!(
                "C frontend dispatch backend override VYRE_BACKEND={id} failed: {error}. Fix: set VYRE_BACKEND to a concrete GPU backend such as `cuda` or `wgpu`, or unset it for CUDA-first selection."
            )
        })?
    } else {
        match dispatch_backend_by_id("cuda") {
            Ok(cuda) => cuda,
            Err(cuda_error) => dispatch_backend_by_id("wgpu").map_err(|wgpu_error| {
                format!(
                    "C frontend dispatch backend unavailable. CUDA-first acquisition failed: {cuda_error}; secondary WGPU GPU backend acquisition failed: {wgpu_error}. Fix: link vyre-driver-cuda or vyre-driver-wgpu and repair GPU driver visibility."
                )
            })?,
        }
    };
    let _ = BACKEND.set(Arc::clone(&backend));
    Ok(BACKEND.get().map_or(backend, Arc::clone))
}

pub(crate) fn dispatch_backend_by_id(id: &str) -> Result<Arc<dyn VyreBackend>, String> {
    if id == "preferred" {
        return shared_dispatch_backend();
    }
    require_gpu_dispatch_backend(id)?;
    static BACKENDS: OnceLock<Mutex<HashMap<String, Arc<dyn VyreBackend>>>> = OnceLock::new();
    let cache = BACKENDS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(backend) = cache
        .lock()
        .map_err(|error| format!("frontend C backend cache lock poisoned: {error}"))?
        .get(id)
        .cloned()
    {
        return Ok(backend);
    }
    let backend = vyre_driver::backend::acquire(id)
        .map_err(|error| format!("dispatch backend `{id}` unavailable: {error}"))?;
    require_gpu_dispatch_backend(backend.id())?;
    let backend: Arc<dyn VyreBackend> = Arc::from(backend);
    cache
        .lock()
        .map_err(|error| {
            format!("frontend C backend cache lock poisoned while inserting: {error}")
        })?
        .insert(id.to_string(), Arc::clone(&backend));
    Ok(backend)
}
