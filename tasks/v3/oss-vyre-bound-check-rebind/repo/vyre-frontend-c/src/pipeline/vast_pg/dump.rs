use std::path::Path;

/// Write the typed VAST blob as JSON for divergence-gate comparison tooling.
pub(super) fn dump_typed_vast_as_json(
    dump_dir: &str,
    source_path: &Path,
    typed_vast_blob: &[u8],
    vast_count: u32,
) -> std::io::Result<()> {
    use std::fs;
    use std::io::Write as _;

    fs::create_dir_all(dump_dir)?;
    let stem = source_path
        .file_name()
        .map(|s| {
            s.to_str().map(str::to_owned).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "VAST dump source basename for {} is not valid UTF-8; lossy dump filenames are forbidden.",
                        source_path.display()
                    ),
                )
            })
        })
        .transpose()?
        .unwrap_or_else(|| "unknown".to_string());
    let out_path = std::path::PathBuf::from(dump_dir).join(format!("{stem}.vast.json"));

    let stride: usize = 10;
    let count = vast_count as usize;
    let mut file = fs::File::create(&out_path)?;
    write!(
        file,
        "{{\"stride\":{stride},\"count\":{count},\"source\":\"{}\",\"nodes\":[",
        source_path.display()
    )?;
    for i in 0..count {
        if i > 0 {
            write!(file, ",")?;
        }
        let base = i * stride * 4;
        write!(file, "[")?;
        for f in 0..stride {
            if f > 0 {
                write!(file, ",")?;
            }
            let off = base + f * 4;
            let word = if off + 4 <= typed_vast_blob.len() {
                u32::from_le_bytes([
                    typed_vast_blob[off],
                    typed_vast_blob[off + 1],
                    typed_vast_blob[off + 2],
                    typed_vast_blob[off + 3],
                ])
            } else {
                0
            };
            write!(file, "{word}")?;
        }
        write!(file, "]")?;
    }
    write!(file, "]}}")?;
    Ok(())
}
