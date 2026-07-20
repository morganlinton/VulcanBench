use crate::report::json::ReportSchema;
use rusqlite::{params, Connection, Result};
use std::path::Path;

pub fn write_sqlite_report(report: &ReportSchema) -> Result<()> {
    let db_path = Path::new(".vyre_bench.db");
    let conn = Connection::open(db_path)?;

    // Setup schema
    conn.execute(
        "CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY,
            run_id TEXT NOT NULL,
            suite TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            total_time_ns INTEGER NOT NULL,
            passed INTEGER NOT NULL,
            failed INTEGER NOT NULL,
            environment_json TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS cases (
            id INTEGER PRIMARY KEY,
            run_id INTEGER NOT NULL,
            case_id TEXT NOT NULL,
            status TEXT NOT NULL,
            correctness TEXT NOT NULL,
            metrics_json TEXT NOT NULL,
            FOREIGN KEY(run_id) REFERENCES runs(id)
        )",
        [],
    )?;

    // Insert run
    let env_json = serde_json::to_string(&report.environment).unwrap_or_default();
    conn.execute(
        "INSERT INTO runs (run_id, suite, total_time_ns, passed, failed, environment_json) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            report.run_id,
            report.suite,
            sqlite_i64(report.summary.total_time_ns),
            sqlite_i64(report.summary.passed as u64),
            sqlite_i64(report.summary.failed as u64),
            env_json
        ],
    )?;

    let run_pk = conn.last_insert_rowid();

    // Insert cases
    for case in &report.cases {
        let metrics_json = serde_json::to_string(&case.metrics).unwrap_or_default();
        let correctness_str = match &case.correctness {
            crate::api::case::Correctness::Exact => "Exact",
            crate::api::case::Correctness::Toleranced { .. } => "Toleranced",
            crate::api::case::Correctness::Certificate { .. } => "Certificate",
            crate::api::case::Correctness::Invalid { .. } => "Invalid",
        };

        conn.execute(
            "INSERT INTO cases (run_id, case_id, status, correctness, metrics_json) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_pk, case.id, case.status, correctness_str, metrics_json],
        )?;
    }

    Ok(())
}

fn sqlite_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
