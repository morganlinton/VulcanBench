use std::fs;
use std::io::{self, Read};
use std::path::Path;

const MAX_SCOPED_CATEGORY_SOURCE_BYTES: u64 = 2_097_152;

pub(crate) fn scoped_category_check(
    op: &crate::quick::quick_op::QuickOp,
    source_file: &Path,
) -> (crate::quick::quick_status::QuickStatus, String) {
    let source = match read_text_bounded(source_file) {
        Ok(source) => source,
        Err(err) => {
            return (
                crate::quick::quick_status::QuickStatus::Fail,
                format!("could not read {}: {err}", source_file.display()),
            );
        }
    };

    if !source.contains(op.id) {
        return (
            crate::quick::quick_status::QuickStatus::Fail,
            format!("{} does not declare {}", source_file.display(), op.id),
        );
    }
    if source.contains("Box<dyn Op>") || source.contains("&dyn Op") {
        return (
            crate::quick::quick_status::QuickStatus::Fail,
            format!(
                "Category B dynamic op dispatch in {}",
                source_file.display()
            ),
        );
    }
    if source.contains("Opcode::") && !source_file.display().to_string().contains("bytecode") {
        return (
            crate::quick::quick_status::QuickStatus::Fail,
            format!("Category B interpreter loop in {}", source_file.display()),
        );
    }

    if source.contains("category_a_self") || source.contains("Category::Intrinsic") {
        (
            crate::quick::quick_status::QuickStatus::Pass,
            format!("category checks scoped to {}", source_file.display()),
        )
    } else {
        (
            crate::quick::quick_status::QuickStatus::Fail,
            "missing Category A/C declaration in op source".to_string(),
        )
    }
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_SCOPED_CATEGORY_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_SCOPED_CATEGORY_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_SCOPED_CATEGORY_SOURCE_BYTES} byte scoped category read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
