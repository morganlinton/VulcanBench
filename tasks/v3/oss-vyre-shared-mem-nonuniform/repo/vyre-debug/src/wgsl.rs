use std::collections::BTreeMap;
use vyre_foundation::ir::Program;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WgslDump {
    pub text: String,
    pub variable_lines: BTreeMap<String, usize>,
}

pub fn dump_wgsl(program: &Program) -> Result<WgslDump, String> {
    let lowered = vyre_lower::lower_for_emit(program).map_err(|e| format!("{:?}", e))?;
    let module = vyre_emit_naga::emit(&lowered.descriptor).map_err(|e| format!("{:?}", e))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| format!("Naga validation failed: {:?}", e))?;

    let mut text = String::new();
    let mut writer =
        naga::back::wgsl::Writer::new(&mut text, naga::back::wgsl::WriterFlags::empty());
    writer
        .write(&module, &info)
        .map_err(|e| format!("WGSL emission failed: {:?}", e))?;

    // Very naive variable line extraction (best effort)
    let mut variable_lines = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        if line.contains("var ") || line.contains("let ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                let name = parts[1].trim_end_matches(':').to_string();
                if !variable_lines.contains_key(&name) {
                    variable_lines.insert(name, i + 1);
                }
            }
        }
    }

    Ok(WgslDump {
        text,
        variable_lines,
    })
}

pub fn dump_wgsl_with_lines(program: &Program) -> Result<WgslDump, String> {
    let dump = dump_wgsl(program)?;
    let mut numbered_text = String::new();
    for (i, line) in dump.text.lines().enumerate() {
        if !line.trim().is_empty() {
            numbered_text.push_str(&format!("{:5} | {}\n", i + 1, line));
        } else {
            numbered_text.push('\n');
        }
    }

    Ok(WgslDump {
        text: numbered_text,
        variable_lines: dump.variable_lines,
    })
}
