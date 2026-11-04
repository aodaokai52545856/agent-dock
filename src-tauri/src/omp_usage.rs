//! Native OMP billable records, deduplicated across forked journals.
use crate::{
    tools::{self, omp},
    usage::UsageRecord,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::PathBuf};

pub fn scan() -> (Vec<UsageRecord>, u32) {
    let mut records = Vec::new();
    let mut files = 0;
    let mut seen = HashSet::new();
    for p in omp::profiles() {
        for root in omp::session_roots(&p.id) {
            let mut paths = Vec::new();
            collect(&root, 0, &mut paths);
            for path in paths {
                files += 1;
                let es = omp::entries(&path);
                let sid = omp::header(&es)
                    .and_then(|h| h["id"].as_str())
                    .unwrap_or("");
                for row in parse(&p.id, sid, &es) {
                    if seen.insert(row.request_id.clone()) {
                        records.push(row);
                    }
                }
            }
        }
    }
    (records, files)
}
fn collect(root: &std::path::Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 16 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            if let Ok(t) = e.file_type() {
                if t.is_dir() {
                    collect(&e.path(), depth + 1, out);
                } else if t.is_file() && e.path().extension().is_some_and(|s| s == "jsonl") {
                    out.push(e.path());
                }
            }
        }
    }
}
pub fn parse(profile: &str, sid: &str, entries: &[Value]) -> Vec<UsageRecord> {
    let mut rows = Vec::new();
    for e in entries {
        let m = if e["type"] == "model_usage" {
            e
        } else {
            &e["message"]
        };
        if e["type"] != "model_usage" && m["role"] != "assistant" {
            continue;
        }
        let u = &m["usage"];
        if !u.is_object() {
            continue;
        }
        let timestamp = tools::parse_time_value(
            e.get("timestamp")
                .or_else(|| m.get("timestamp"))
                .unwrap_or(&Value::Null),
        )
        .unwrap_or(0)
            / 1000;
        let identity = m["responseId"].as_str().or_else(|| e["id"].as_str());
        let Some(identity) = identity else { continue };
        let provider = m["provider"].as_str().unwrap_or("unknown");
        let model = m["model"].as_str().unwrap_or("unknown");
        let request_id = format!(
            "omp:{:x}",
            Sha256::digest(format!("{profile}\0{provider}\0{model}\0{identity}"))
        );
        let count = |name: &str| u[name].as_u64().unwrap_or(0);
        let cost = u["cost"]["total"]
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0);
        rows.push(UsageRecord {
            request_id,
            created_at: timestamp,
            app_filter: format!("omp:{profile}"),
            app_label: format!("OMP · {profile}"),
            provider_name: provider.into(),
            model: model.into(),
            session_id: omp::public_id(profile, sid),
            input_tokens: count("input"),
            output_tokens: count("output"),
            cache_read_tokens: count("cacheRead"),
            cache_creation_tokens: count("cacheWrite"),
            cost_usd: cost.unwrap_or(0.0),
            unpriced: cost.is_none(),
            status_code: if m["stopReason"] == "error" { 500 } else { 200 },
            data_source: "omp_native".into(),
        });
    }
    rows
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn native_usage_keeps_cache_cost_and_stable_fork_identity() {
        let e = json!({"id":"entry","timestamp":"2026-11-02T01:00:00Z","type":"message","message":{"role":"assistant","provider":"p","model":"m","usage":{"input":3,"output":5,"cacheRead":7,"cacheWrite":11,"cost":{"total":0.123}}}});
        let a = parse("work", "one", &[e.clone()]);
        let b = parse("work", "fork", &[e.clone()]);
        let c = parse("private", "one", &[e]);
        assert_eq!(a[0].request_id, b[0].request_id);
        assert_ne!(a[0].request_id, c[0].request_id);
        assert_eq!(a[0].cache_creation_tokens, 11);
        assert_eq!(a[0].cost_usd, 0.123);
        assert!(!a[0].unpriced);
    }
    #[test]
    fn auxiliary_model_usage_is_included_and_missing_price_is_explicit() {
        let rows = parse(
            "default",
            "s",
            &[
                json!({"id":"u","type":"model_usage","purpose":"compaction","provider":"p","model":"m","usage":{"input":3,"output":2}}),
            ],
        );
        assert_eq!(rows.len(), 1);
        assert!(rows[0].unpriced);
    }
}
