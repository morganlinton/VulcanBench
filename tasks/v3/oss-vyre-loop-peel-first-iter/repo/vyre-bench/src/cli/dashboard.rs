use crate::report::json::ReportSchema;
use super::load_report;

pub(super) fn generate_dashboard(output_dir: impl AsRef<str>) -> anyhow::Result<()> {
    let output = std::path::Path::new(output_dir.as_ref());
    std::fs::create_dir_all(output)?;
    std::fs::create_dir_all(output.join("data"))?;
    std::fs::create_dir_all(output.join("history"))?;

    // Find latest snapshot
    let snapshots_dir = std::path::Path::new("snapshots");
    let latest = find_latest_snapshot(snapshots_dir)?;
    let report: ReportSchema = load_report(&latest.to_string_lossy())?;

    // Copy raw data
    std::fs::copy(&latest, output.join("data/results.json"))?;

    // Generate scorecard markdown
    let scorecard_md = generate_scorecard_md(&report);
    std::fs::write(output.join("scorecard.md"), &scorecard_md)?;

    // Generate per-case SVG bar charts
    for case in &report.cases {
        let svg = generate_case_svg(case);
        let filename = case.id.replace('.', "_") + ".svg";
        std::fs::write(output.join(&filename), &svg)?;
    }

    // Generate cross-backend SVG
    let cross_svg = generate_cross_backend_svg(&report);
    std::fs::write(output.join("cross-backend.svg"), &cross_svg)?;

    // Generate index.html
    let html = generate_index_html(&report, &scorecard_md);
    std::fs::write(output.join("index.html"), &html)?;

    println!(
        "Dashboard generated: {} ({} cases, {} files)",
        output.display(),
        report.cases.len(),
        4 + report.cases.len() // index.html + scorecard.md + data/results.json + cross-backend.svg + per-case SVGs
    );
    Ok(())
}

fn find_latest_snapshot(dir: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    if !dir.exists() {
        anyhow::bail!("snapshots directory does not exist: {}", dir.display());
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| {
        e.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    entries
        .last()
        .map(|e| e.path())
        .ok_or_else(|| anyhow::anyhow!("no snapshot files found in {}", dir.display()))
}

pub(super) fn generate_scorecard_md(report: &ReportSchema) -> String {
    let mut md = String::new();
    md.push_str("# vyre-bench Scorecard\n\n");
    md.push_str(&format!(
        "Suite: **{}** | Cases: {}/{} passed\n\n",
        report.suite,
        report
            .cases
            .iter()
            .filter(|case| case.passes_summary_evidence())
            .count(),
        report.cases.len(),
    ));
    md.push_str("| Case | Status | p50 (ns) | p99 (ns) | Speedup | CV |\n");
    md.push_str("|------|--------|----------|----------|---------|----|\n");
    for case in &report.cases {
        let wall = case.metrics.get("wall_ns");
        let p50 = wall.map(|s| s.p50).unwrap_or(0);
        let p99 = wall.map(|s| s.p99).unwrap_or(0);
        let cv = wall
            .map(|s| {
                if s.mean > 0.0 {
                    format!("{:.3}", s.stddev / s.mean)
                } else {
                    " - ".into()
                }
            })
            .unwrap_or_else(|| " - ".into());
        let speedup = case
            .performance
            .as_ref()
            .and_then(|p| p.speedup_x)
            .map(|s| format!("{:.1}×", s))
            .unwrap_or_else(|| " - ".into());
        let status_emoji = if case.passes_summary_evidence() {
            "✅"
        } else {
            match case.status.as_str() {
                "failed" => "❌",
                "unstable" | "thermal_unstable" => "⚠️",
                _ => "❓",
            }
        };
        md.push_str(&format!(
            "| {} | {} {} | {:>10} | {:>10} | {:>7} | {} |\n",
            case.id, status_emoji, case.status, p50, p99, speedup, cv
        ));
    }
    md
}

fn generate_case_svg(case: &crate::report::json::CaseReport) -> String {
    let wall = case.metrics.get("wall_ns");
    let p50 = wall.map(|s| s.p50).unwrap_or(1) as f64;
    let p99 = wall.map(|s| s.p99).unwrap_or(1) as f64;
    let max = wall.map(|s| s.max).unwrap_or(1) as f64;
    let scale = 300.0 / max.max(1.0);

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="80" viewBox="0 0 400 80">
  <style>
    text {{ font-family: 'Inter', sans-serif; font-size: 11px; fill: #e0e0e0; }}
    .title {{ font-size: 12px; font-weight: 600; }}
    .bar {{ rx: 3; }}
  </style>
  <rect width="400" height="80" fill="#1a1a2e" rx="6"/>
  <text x="10" y="16" class="title">{id}</text>
  <rect class="bar" x="10" y="28" width="{w50}" height="14" fill="#00d2ff"/>
  <text x="{tw50}" y="39" fill="#fff">p50: {p50_ns}ns</text>
  <rect class="bar" x="10" y="48" width="{w99}" height="14" fill="#7b2ff7"/>
  <text x="{tw99}" y="59" fill="#fff">p99: {p99_ns}ns</text>
  <rect class="bar" x="10" y="68" width="{wmax}" height="8" fill="#ff6b6b" opacity="0.5"/>
</svg>"##,
        id = case.id,
        w50 = (p50 * scale) as u32,
        w99 = (p99 * scale) as u32,
        wmax = (max * scale) as u32,
        tw50 = (p50 * scale) as u32 + 14,
        tw99 = (p99 * scale) as u32 + 14,
        p50_ns = p50 as u64,
        p99_ns = p99 as u64,
    )
}

fn generate_cross_backend_svg(report: &ReportSchema) -> String {
    let case_count = report.cases.len();
    let height = 40 + case_count * 30;
    let mut bars = String::new();

    for (i, case) in report.cases.iter().enumerate() {
        let wall = case.metrics.get("wall_ns");
        let p50 = wall.map(|s| s.p50).unwrap_or(0);
        let y = 30 + i * 30;
        let width = (p50 as f64 / 1_000_000.0).clamp(5.0, 350.0) as u32; // scale to ms

        bars.push_str(&format!(
            r##"  <rect x="10" y="{y}" width="{w}" height="20" fill="#00d2ff" rx="3"/>
  <text x="{tx}" y="{ty}" fill="#e0e0e0" font-size="10">{id} ({p50_us}μs)</text>
"##,
            y = y,
            w = width,
            tx = width + 14,
            ty = y + 14,
            id = case.id,
            p50_us = p50 / 1000,
        ));
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="{h}" viewBox="0 0 600 {h}">
  <style>text {{ font-family: 'Inter', sans-serif; }}</style>
  <rect width="600" height="{h}" fill="#1a1a2e" rx="6"/>
  <text x="10" y="20" fill="#e0e0e0" font-size="14" font-weight="600">Cross-Backend: {suite}</text>
{bars}</svg>"##,
        h = height,
        suite = report.suite,
        bars = bars,
    )
}

pub(super) fn generate_index_html(report: &ReportSchema, _scorecard_md: &str) -> String {
    let cases_count = report.cases.len();
    let passed = report
        .cases
        .iter()
        .filter(|case| case.passes_summary_evidence())
        .count();

    let mut rows = String::new();
    for case in &report.cases {
        let wall = case.metrics.get("wall_ns");
        let p50 = wall.map(|s| s.p50).unwrap_or(0);
        let p99 = wall.map(|s| s.p99).unwrap_or(0);
        let cv = wall
            .map(|s| {
                if s.mean > 0.0 {
                    format!("{:.3}", s.stddev / s.mean)
                } else {
                    " - ".into()
                }
            })
            .unwrap_or_else(|| " - ".into());
        let speedup = case
            .performance
            .as_ref()
            .and_then(|p| p.speedup_x)
            .map(|s| format!("{:.1}×", s))
            .unwrap_or_else(|| " - ".into());
        let status_class = if case.passes_summary_evidence() {
            "status-pass"
        } else {
            match case.status.as_str() {
                "failed" => "status-fail",
                _ => "status-warn",
            }
        };
        let svg_file = case.id.replace('.', "_") + ".svg";

        rows.push_str(&format!(
            r#"        <tr>
          <td><a href="{svg}">{id}</a></td>
          <td class="{cls}">{status}</td>
          <td class="num">{p50}</td>
          <td class="num">{p99}</td>
          <td class="num">{speedup}</td>
          <td class="num">{cv}</td>
        </tr>
"#,
            svg = svg_file,
            id = case.id,
            cls = status_class,
            status = case.status,
            p50 = p50,
            p99 = p99,
            speedup = speedup,
            cv = cv,
        ));
    }

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>vyre-bench Dashboard</title>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg: #0f0f23;
      --surface: #1a1a2e;
      --accent: #00d2ff;
      --accent2: #7b2ff7;
      --text: #e0e0e0;
      --pass: #00e676;
      --fail: #ff5252;
      --warn: #ffab40;
    }}
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{
      font-family: 'Inter', sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
      padding: 2rem;
    }}
    h1 {{
      font-size: 2rem;
      background: linear-gradient(135deg, var(--accent), var(--accent2));
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      margin-bottom: 0.5rem;
    }}
    .summary {{
      display: flex;
      gap: 2rem;
      margin: 1rem 0 2rem;
    }}
    .stat {{
      background: var(--surface);
      border-radius: 12px;
      padding: 1.5rem 2rem;
      min-width: 140px;
      text-align: center;
    }}
    .stat-value {{
      font-size: 2.5rem;
      font-weight: 700;
      color: var(--accent);
    }}
    .stat-label {{
      font-size: 0.8rem;
      text-transform: uppercase;
      letter-spacing: 0.1em;
      opacity: 0.7;
    }}
    table {{
      width: 100%;
      border-collapse: collapse;
      background: var(--surface);
      border-radius: 12px;
      overflow: hidden;
    }}
    th {{
      text-align: left;
      padding: 0.8rem 1rem;
      font-size: 0.75rem;
      text-transform: uppercase;
      letter-spacing: 0.1em;
      border-bottom: 1px solid rgba(255,255,255,0.1);
      background: rgba(0,0,0,0.2);
    }}
    td {{
      padding: 0.6rem 1rem;
      border-bottom: 1px solid rgba(255,255,255,0.05);
    }}
    td a {{
      color: var(--accent);
      text-decoration: none;
    }}
    td a:hover {{ text-decoration: underline; }}
    .num {{ font-variant-numeric: tabular-nums; text-align: right; }}
    .status-pass {{ color: var(--pass); font-weight: 600; }}
    .status-fail {{ color: var(--fail); font-weight: 600; }}
    .status-warn {{ color: var(--warn); font-weight: 600; }}
    .footer {{
      margin-top: 2rem;
      font-size: 0.8rem;
      opacity: 0.5;
    }}
    tr:hover {{ background: rgba(0,210,255,0.05); }}
  </style>
</head>
<body>
  <h1>vyre-bench Dashboard</h1>
  <p>Suite: <strong>{suite}</strong> &mdash; Generated {timestamp}</p>

  <div class="summary">
    <div class="stat">
      <div class="stat-value">{passed}</div>
      <div class="stat-label">Passed</div>
    </div>
    <div class="stat">
      <div class="stat-value">{total}</div>
      <div class="stat-label">Total Cases</div>
    </div>
    <div class="stat">
      <div class="stat-value">{pass_rate}%</div>
      <div class="stat-label">Pass Rate</div>
    </div>
  </div>

  <table>
    <thead>
      <tr>
        <th>Case</th>
        <th>Status</th>
        <th>p50 (ns)</th>
        <th>p99 (ns)</th>
        <th>Speedup</th>
        <th>CV</th>
      </tr>
    </thead>
    <tbody>
{rows}    </tbody>
  </table>

  <div class="footer">
    <p>Data: <a href="data/results.json">results.json</a> |
       Cross-backend: <a href="cross-backend.svg">cross-backend.svg</a> |
       Scorecard: <a href="scorecard.md">scorecard.md</a></p>
  </div>
</body>
</html>"##,
        suite = report.suite,
        timestamp = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            format!("{now} (unix)")
        },
        passed = passed,
        total = cases_count,
        pass_rate = if cases_count > 0 {
            passed * 100 / cases_count
        } else {
            0
        },
        rows = rows,
    )
}
