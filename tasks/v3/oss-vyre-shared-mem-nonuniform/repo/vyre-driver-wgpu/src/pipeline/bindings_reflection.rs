//! WGSL bind-group reflection used by the reusable pipeline wrapper.

/// Return every `(group, binding)` pair declared by lowered WGSL.
///
/// The core lowerer only emits resources the shader actually uses. The
/// wgpu pipeline wrapper must mirror that layout exactly when creating
/// bind groups; extra entries are validation errors.
///
/// Before 0.6 this scanner only found `@group(0)` bindings. With the
/// bind-group policy opened up (see `lowering::bind_group_for`) a
/// future lowering that partitions bindings across multiple groups
/// must still reflect correctly. The generalised scanner returns
/// every `(group, binding)` pair found in the WGSL and sorts them so
/// the wgpu `BindGroupLayoutDescriptor` can be built deterministically.
#[must_use]
pub(crate) fn declared_bindings(wgsl: &str) -> Vec<(u32, u32)> {
    let mut bindings: Vec<(u32, u32)> = Vec::with_capacity(4);
    let mut rest = wgsl;
    while let Some(group_pos) = rest.find("@group(") {
        rest = &rest[group_pos + "@group(".len()..];
        let Some(group_end) = rest.find(')') else {
            break;
        };
        let Ok(group) = rest[..group_end].trim().parse::<u32>() else {
            rest = &rest[group_end + 1..];
            continue;
        };
        rest = &rest[group_end + 1..];
        let Some(binding_pos) = rest.find("@binding(") else {
            continue;
        };
        rest = &rest[binding_pos + "@binding(".len()..];
        let Some(end) = rest.find(')') else {
            break;
        };
        if let Ok(binding) = rest[..end].trim().parse::<u32>() {
            let entry = (group, binding);
            if !bindings.contains(&entry) {
                bindings.push(entry);
            }
        }
        rest = &rest[end + 1..];
    }
    bindings.sort_unstable();
    bindings
}

#[cfg(test)]
mod tests {
    use super::declared_bindings;

    #[test]
    fn group_zero_only_shader() {
        let wgsl = "@group(0) @binding(0) var<storage, read> a: u32; @group(0) @binding(1) var<storage, read_write> b: u32;";
        assert_eq!(declared_bindings(wgsl), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn multi_group_shader() {
        let wgsl =
            "@group(0) @binding(0) var<storage> a: u32; @group(1) @binding(2) var<uniform> b: u32;";
        assert_eq!(declared_bindings(wgsl), vec![(0, 0), (1, 2)]);
    }

    #[test]
    fn deduplicates_entries() {
        let wgsl =
            "@group(0) @binding(0) var<storage> a: u32; @group(0) @binding(0) var<storage> b: u32;";
        assert_eq!(declared_bindings(wgsl), vec![(0, 0)]);
    }
}
