//! Local Antigravity usage from exported session counters.

use crate::{tools::parse_time_value, usage::UsageRecord};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

pub(crate) const APP_ID: &str = "antigravity";
pub(crate) const APP_LABEL: &str = "Antigravity";
pub(crate) const SESSION_PROVIDER: &str = "Antigravity · 会话日志";
const NATIVE_ROOTS: &[&str] = &[
    "antigravity-cli",
    "antigravity",
    "antigravity-ide",
    "antigravity-acp",
];

#[derive(PartialEq, Eq)]
struct FileStamp {
    length: u64,
    modified: SystemTime,
}

struct CachedTranscript {
    stamp: Option<FileStamp>,
    records: Vec<UsageRecord>,
}

type TranscriptCache = HashMap<PathBuf, CachedTranscript>;
static TRANSCRIPTS: LazyLock<Mutex<TranscriptCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn scan() -> (Vec<UsageRecord>, u32) {
    let Some(home) = dirs::home_dir() else {
        return (Vec::new(), 0);
    };
    let mut cache = TRANSCRIPTS.lock().unwrap_or_else(|err| err.into_inner());
    scan_native(&home.join(".gemini"), &mut cache)
}

fn scan_native(gemini: &Path, cache: &mut TranscriptCache) -> (Vec<UsageRecord>, u32) {
    let mut paths = HashSet::new();
    let mut ids = HashSet::new();
    let mut records = Vec::new();
    for app in NATIVE_ROOTS {
        let brain = gemini.join(app).join("brain");
        let Ok(canonical_brain) = brain.canonicalize() else {
            continue;
        };
        let Ok(sessions) = fs::read_dir(&brain) else {
            continue;
        };
        for session in sessions.flatten() {
            if !session.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let Some(id) = session
                .file_name()
                .into_string()
                .ok()
                .filter(|id| Uuid::parse_str(id).is_ok())
            else {
                continue;
            };
            let logs = session.path().join(".system_generated").join("logs");
            let Some(path) = ["transcript_full.jsonl", "transcript.jsonl"]
                .into_iter()
                .filter_map(|file| logs.join(file).canonicalize().ok())
                .find(|path| path.starts_with(&canonical_brain) && path.is_file())
            else {
                continue;
            };
            if !paths.insert(path.clone()) {
                continue;
            }
            let stamp = fs::metadata(&path).ok().and_then(|meta| {
                Some(FileStamp {
                    length: meta.len(),
                    modified: meta.modified().ok()?,
                })
            });
            let unchanged =
                stamp.is_some() && cache.get(&path).is_some_and(|entry| entry.stamp == stamp);
            if !unchanged {
                if let Ok(rows) = read_transcript(&path, &id) {
                    cache.insert(
                        path.clone(),
                        CachedTranscript {
                            stamp,
                            records: rows,
                        },
                    );
                }
            }
            if let Some(entry) = cache.get(&path) {
                records.extend(
                    entry
                        .records
                        .iter()
                        .filter(|row| ids.insert(row.request_id.clone()))
                        .cloned(),
                );
            }
        }
    }
    cache.retain(|path, _| paths.contains(path));
    (records, paths.len() as u32)
}

#[derive(Deserialize)]
struct TranscriptStep {
    step_index: u64,
    #[serde(rename = "type")]
    step_type: String,
    status: String,
    #[serde(default)]
    created_at: Value,
    model: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

fn read_transcript(path: &Path, session: &str) -> std::io::Result<Vec<UsageRecord>> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut line = Vec::new();
    let mut steps = BTreeMap::new();
    while reader.read_until(b'\n', &mut line)? != 0 {
        if let Ok(step) = serde_json::from_slice::<TranscriptStep>(&line) {
            // agy can write a revised status or usage for an existing step.
            steps.insert(step.step_index, step.into_record(session));
        }
        line.clear();
    }
    Ok(steps.into_values().flatten().collect())
}

impl TranscriptStep {
    fn into_record(self, session: &str) -> Option<UsageRecord> {
        if self.step_type != "PLANNER_RESPONSE" || self.status != "DONE" {
            return None;
        }
        let created_at = parse_time_value(&self.created_at)? / 1000;
        // agy's input excludes cached tokens. Its output already includes thinking.
        let input = self.input_tokens.unwrap_or_default();
        let output = self.output_tokens.unwrap_or_default();
        let cache_read = self.cache_read_tokens.unwrap_or_default();
        let cache_write = self.cache_write_tokens.unwrap_or_default();
        if created_at <= 0 || input.max(output).max(cache_read).max(cache_write) == 0 {
            return None;
        }
        let model = match self.model.as_deref().unwrap_or_default().trim() {
            "" => "unknown".into(),
            name if name.contains(' ') => name
                .to_ascii_lowercase()
                .replace(['(', ')'], "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("-"),
            name => name.into(),
        };
        Some(UsageRecord {
            // Same identity as CC Switch's session_usage_agy importer.
            request_id: format!("agy:{session}:{}", self.step_index),
            created_at,
            app_filter: APP_ID.into(),
            app_label: APP_LABEL.into(),
            provider_name: SESSION_PROVIDER.into(),
            model,
            session_id: session.into(),
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_write,
            cost_usd: 0.0,
            unpriced: true,
            status_code: 200,
            data_source: "antigravity_local".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION: &str = "aaaaaaaa-1111-2222-3333-444444444444";

    #[test]
    fn reads_completed_counters_once_across_transcript_mirrors_and_refreshes_changed_files() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir
            .path()
            .join("antigravity-cli/brain")
            .join(SESSION)
            .join(".system_generated/logs");
        fs::create_dir_all(&logs).unwrap();
        let raw = concat!(
            "{\"step_index\":1,\"type\":\"PLANNER_RESPONSE\",\"status\":\"DONE\",\"created_at\":\"2026-11-22T09:30:00Z\",\"input_tokens\":8}\n",
            "{\"step_index\":1,\"type\":\"PLANNER_RESPONSE\",\"status\":\"DONE\",\"created_at\":\"2026-11-22T09:30:00Z\",\"model\":\"Gemini 4 Argon (High)\",\"input_tokens\":10,\"output_tokens\":7,\"cache_read_tokens\":5,\"cache_write_tokens\":2}\n",
            "{\"step_index\":2,\"type\":\"PLANNER_RESPONSE\",\"status\":\"ERROR\",\"input_tokens\":9}\n",
        );
        let path = logs.join("transcript_full.jsonl");
        fs::write(&path, raw).unwrap();
        fs::write(logs.join("transcript.jsonl"), raw).unwrap();
        let mut cache = HashMap::new();
        let (rows, files) = scan_native(dir.path(), &mut cache);
        assert_eq!(files, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].request_id, format!("agy:{SESSION}:1"));
        assert_eq!(rows[0].model, "gemini-4-argon-high");
        assert_eq!(rows[0].input_tokens, 10);
        assert_eq!(crate::usage::total_tokens(&rows[0]), 24);
        assert!(rows[0].unpriced);

        fs::write(
            &path,
            format!(
                "{raw}{{\"step_index\":1,\"type\":\"PLANNER_RESPONSE\",\"status\":\"RUNNING\"}}\n"
            ),
        )
        .unwrap();
        assert!(scan_native(dir.path(), &mut cache).0.is_empty());
    }
}
