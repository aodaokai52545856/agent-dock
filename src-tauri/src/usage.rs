//! Usage dashboard: CC Switch request logs plus extra Claude account jsonl.

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

const MAX_WALK_DEPTH: usize = 16;
const BOUNDARY_TYPE: &str = "agent-dock-copy-boundary";
const CONTINUE_TITLE_MARK: &str = " · 续接";
/// File birth time can land a second after `Utc::now()` captured for the first
/// new turn; inherited jsonl timestamps are hours or days older.
const CONTINUE_CUT_SLACK_SECS: i64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraClaudeProfile {
    pub id: String,
    pub name: String,
    pub email: String,
    pub config_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    pub request_id: String,
    pub created_at: i64,
    pub app_filter: String,
    pub app_label: String,
    pub provider_name: String,
    pub model: String,
    pub session_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
    pub unpriced: bool,
    pub status_code: i64,
    pub data_source: String,
}

pub fn extra_app_label(name: &str, email: &str, id: &str) -> String {
    let name = name.trim();
    if !name.is_empty() {
        return format!("Claude · {name}");
    }
    let email = email.trim();
    if !email.is_empty() {
        return format!("Claude · {email}");
    }
    let short = if id.chars().count() > 8 {
        id.chars().take(8).collect::<String>()
    } else {
        id.to_string()
    };
    format!("Claude · 环境{short}")
}

pub fn scan_extra_profiles(profiles: &[ExtraClaudeProfile]) -> Vec<UsageRecord> {
    scan_extra_profiles_filtered(profiles, &HashMap::new())
}

pub fn scan_extra_profiles_filtered(
    profiles: &[ExtraClaudeProfile],
    ledger: &HashMap<String, i64>,
) -> Vec<UsageRecord> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for profile in profiles {
        for record in scan_profile(profile, ledger) {
            if seen.insert(record.request_id.clone()) {
                out.push(record);
            }
        }
    }
    out
}

fn scan_profile(profile: &ExtraClaudeProfile, ledger: &HashMap<String, i64>) -> Vec<UsageRecord> {
    let mut files = Vec::new();
    collect_jsonl(&profile.config_dir.join("projects"), 0, &mut files);
    let mut cuts = ledger.clone();
    for path in &files {
        let sid = session_id_for_jsonl(path);
        if cuts.contains_key(&sid) {
            continue;
        }
        if jsonl_has_continue_title(path) {
            if let Some(ts) = file_created_unix(path) {
                cuts.insert(sid, ts.saturating_sub(CONTINUE_CUT_SLACK_SECS));
            }
        }
    }
    let label = extra_app_label(&profile.name, &profile.email, &profile.id);
    let mut records = Vec::new();
    for path in files {
        let cut = cuts.get(&session_id_for_jsonl(&path)).copied();
        records.extend(parse_jsonl(profile, &label, &path, cut));
    }
    records
}

fn session_id_for_jsonl(path: &Path) -> String {
    for ancestor in path.ancestors().take(6) {
        if ancestor == path {
            continue;
        }
        if let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) {
            if Uuid::parse_str(name).is_ok() {
                return name.to_string();
            }
        }
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn jsonl_has_continue_title(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("custom-title") {
            if value
                .get("customTitle")
                .and_then(Value::as_str)
                .is_some_and(|title| title.contains(CONTINUE_TITLE_MARK))
            {
                return true;
            }
        }
    }
    false
}

fn file_created_unix(path: &Path) -> Option<i64> {
    let created = fs::metadata(path).ok()?.created().ok()?;
    Some(created.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64)
}

pub fn load_copy_ledger(path: Option<&Path>) -> HashMap<String, i64> {
    let Some(path) = path else {
        return HashMap::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn collect_jsonl(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_WALK_DEPTH || !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_jsonl(&path, depth + 1, out);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || !name.ends_with(".jsonl") {
            continue;
        }
        out.push(path);
    }
}

fn parse_jsonl(
    profile: &ExtraClaudeProfile,
    label: &str,
    path: &Path,
    cut: Option<i64>,
) -> Vec<UsageRecord> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let fallback_session = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let mut pending = Vec::new();
    let mut after_boundary = Vec::new();
    let mut saw_boundary = false;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some(BOUNDARY_TYPE) {
            saw_boundary = true;
            pending.clear();
            continue;
        }
        if let Some(record) = assistant_record(profile, label, &fallback_session, &line, &value) {
            if saw_boundary {
                after_boundary.push(record);
            } else {
                pending.push(record);
            }
        }
    }
    let rows = if saw_boundary {
        after_boundary
    } else {
        pending
    };
    match cut {
        Some(ts) => rows.into_iter().filter(|row| row.created_at >= ts).collect(),
        None => rows,
    }
}

fn assistant_record(
    profile: &ExtraClaudeProfile,
    label: &str,
    fallback_session: &str,
    line: &str,
    value: &Value,
) -> Option<UsageRecord> {
    if value.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let usage = value.pointer("/message/usage")?;
    let created_at = parse_timestamp(value.get("timestamp")).filter(|ts| *ts != 0)?;
    let session_id = value
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_session.to_string());
    let uuid = value
        .get("uuid")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/message/id").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| stable_hash(line));
    let model = value
        .pointer("/message/model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(UsageRecord {
        request_id: format!("claude_extra:{}:{}:{}", profile.id, session_id, uuid),
        created_at,
        app_filter: format!("claude:{}", profile.id),
        app_label: label.to_string(),
        provider_name: label.to_string(),
        model,
        session_id,
        input_tokens: json_u64(usage.get("input_tokens")),
        output_tokens: json_u64(usage.get("output_tokens")),
        cache_read_tokens: json_u64(usage.get("cache_read_input_tokens")),
        cache_creation_tokens: json_u64(usage.get("cache_creation_input_tokens")),
        cost_usd: 0.0,
        unpriced: true,
        status_code: 200,
        data_source: "claude_extra".into(),
    })
}

fn json_u64(value: Option<&Value>) -> u64 {
    value
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().and_then(|n| u64::try_from(n).ok())))
        .unwrap_or(0)
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    if let Some(n) = value.as_f64() {
        return Some(n as i64);
    }
    let s = value.as_str()?;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

fn stable_hash(line: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(line.as_bytes());
    format!("{:x}", hasher.finalize())
}

const HOURLY_RANGE_SECS: i64 = 36 * 3600;
const MSG_MISSING: &str = "请先打开 CC Switch，让它扫描会话日志";
const MSG_BUSY: &str = "用量库正被 CC Switch 占用，请稍后重试";
const MSG_SCHEMA: &str = "这份 CC Switch 数据库对不上，请升级 CC Switch";
const MSG_EXPORT_NONE: &str = "没有可同步的副号用量";
const MSG_EXPORT_DONE: &str = "副号用量已在 CC Switch 里，没有新记录";
const LOG_SELECT: &str = "SELECT request_id, created_at, app_type, provider_id, model,
    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
    total_cost_usd, session_id, status_code, data_source
    FROM proxy_request_logs";
const EXPORT_INSERT: &str = "INSERT OR IGNORE INTO proxy_request_logs (
    request_id, provider_id, app_type, model,
    input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
    input_token_semantics, total_cost_usd, latency_ms, status_code,
    session_id, created_at, data_source
) VALUES (?1, ?2, 'claude', ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12, 'claude_extra')";
const PRICE_SELECT: &str = "SELECT model_id, display_name, input_cost_per_million, output_cost_per_million,
    cache_read_cost_per_million, cache_creation_cost_per_million FROM model_pricing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbStatus {
    Ok,
    Missing,
    Busy,
    Schema,
}

#[derive(Debug, Clone)]
pub struct PricingRow {
    pub model_id: String,
    pub display_name: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_creation_per_million: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageQuery {
    pub start: i64,
    pub end: i64,
    pub app_filter: String,
    #[serde(default)]
    pub provider_name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageAppChip {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsagePoint {
    pub ts: i64,
    pub label: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub tokens: u64,
    pub cost_usd: f64,
    pub requests: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageHeatCell {
    pub ts: i64,
    pub label: String,
    pub tokens: u64,
    pub cost_usd: f64,
    pub requests: u32,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageGroupRow {
    pub name: String,
    pub request_count: u32,
    pub tokens: u64,
    pub cost_usd: f64,
    pub unpriced: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageDashboard {
    pub ok: bool,
    pub db_ok: bool,
    pub message: Option<String>,
    pub total_cost_usd: f64,
    pub total_requests: u32,
    pub total_tokens: u64,
    pub cache_hit_percent: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub points: Vec<UsagePoint>,
    pub heatmap: Vec<UsageHeatCell>,
    pub providers: Vec<UsageGroupRow>,
    pub models: Vec<UsageGroupRow>,
    pub apps: Vec<UsageAppChip>,
    pub pricing: Vec<UsagePricingView>,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsagePricingView {
    pub model_id: String,
    pub display_name: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_creation_per_million: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageLogPage {
    pub total: u32,
    pub page: u32,
    pub page_size: u32,
    pub rows: Vec<UsageRecord>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageSyncResult {
    pub imported: u32,
    pub files_scanned: u32,
    pub extra_accounts: u32,
    pub db_ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageExportResult {
    pub written: u32,
    pub skipped: u32,
    pub extra_accounts: u32,
    pub db_ok: bool,
    pub message: Option<String>,
}

pub fn ccswitch_db_path() -> Option<PathBuf> {
    let path = dirs::home_dir()?.join(".cc-switch").join("cc-switch.db");
    path.is_file().then_some(path)
}

pub fn db_status_message(status: DbStatus) -> Option<String> {
    match status {
        DbStatus::Ok => None,
        DbStatus::Missing => Some(MSG_MISSING.into()),
        DbStatus::Busy => Some(MSG_BUSY.into()),
        DbStatus::Schema => Some(MSG_SCHEMA.into()),
    }
}

pub fn read_ccswitch_db(path: &Path) -> Result<(Vec<UsageRecord>, Vec<PricingRow>), DbStatus> {
    if !path.is_file() {
        return Err(DbStatus::Missing);
    }
    let conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| classify_db_error(&err))?;
    conn.busy_timeout(std::time::Duration::from_millis(2_000))
        .map_err(|err| classify_db_error(&err))?;
    let logs = read_logs(&conn)?;
    let pricing = read_pricing(&conn).unwrap_or_default();
    Ok((logs, pricing))
}

fn classify_db_error(err: &rusqlite::Error) -> DbStatus {
    match err {
        rusqlite::Error::SqliteFailure(info, _)
            if info.code == rusqlite::ErrorCode::DatabaseBusy
                || info.code == rusqlite::ErrorCode::DatabaseLocked =>
        {
            DbStatus::Busy
        }
        rusqlite::Error::InvalidColumnName(_) => DbStatus::Schema,
        other if other.to_string().contains("no such column") || other.to_string().contains("no such table") => {
            DbStatus::Schema
        }
        _ => DbStatus::Schema,
    }
}

fn read_logs(conn: &rusqlite::Connection) -> Result<Vec<UsageRecord>, DbStatus> {
    let mut stmt = conn.prepare(LOG_SELECT).map_err(|err| classify_db_error(&err))?;
    let rows = stmt
        .query_map([], |row| {
            let request_id: String = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            let app_type: String = row.get(2)?;
            let provider_id: String = row.get(3)?;
            let model: String = row.get(4)?;
            let input: i64 = row.get(5)?;
            let output: i64 = row.get(6)?;
            let cache_read: i64 = row.get(7)?;
            let cache_creation: i64 = row.get(8)?;
            let cost: String = row.get(9)?;
            let session_id: Option<String> = row.get(10)?;
            let status_code: i64 = row.get(11)?;
            let data_source: String = row.get(12)?;
            Ok(UsageRecord {
                request_id,
                created_at,
                app_filter: app_type.clone(),
                app_label: app_type_label(&app_type),
                provider_name: if provider_id.is_empty() {
                    format!("{} · 会话日志", app_type_label(&app_type))
                } else {
                    provider_id
                },
                model,
                session_id: session_id.unwrap_or_default(),
                input_tokens: input.max(0) as u64,
                output_tokens: output.max(0) as u64,
                cache_read_tokens: cache_read.max(0) as u64,
                cache_creation_tokens: cache_creation.max(0) as u64,
                cost_usd: cost.parse().unwrap_or(0.0),
                unpriced: false,
                status_code,
                data_source,
            })
        })
        .map_err(|err| classify_db_error(&err))?;
    let mut out = Vec::new();
    for row in rows {
        match row {
            Ok(record) => {
                if is_dock_extra_row(&record) {
                    continue;
                }
                out.push(record);
            }
            Err(_) => continue,
        }
    }
    Ok(out)
}

fn is_dock_extra_row(record: &UsageRecord) -> bool {
    record.data_source == "claude_extra" || record.request_id.starts_with("claude_extra:")
}

fn cost_text(value: f64) -> String {
    if !value.is_finite() || value == 0.0 {
        return "0".into();
    }
    let raw = format!("{value:.8}");
    let trimmed = raw.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".into()
    } else {
        trimmed.to_string()
    }
}

fn extra_account_count(extras: &[UsageRecord]) -> u32 {
    extras.iter().map(|row| row.app_filter.as_str()).collect::<HashSet<_>>().len() as u32
}

pub fn export_message(written: u32, skipped: u32, extra_len: usize) -> String {
    if extra_len == 0 {
        return MSG_EXPORT_NONE.into();
    }
    if written == 0 {
        return MSG_EXPORT_DONE.into();
    }
    format!(
        "已把 {written} 条副号用量写入 CC Switch（跳过 {skipped} 条已有记录）。用量会出现在 Claude 应用下，供应商按账号分开。"
    )
}

pub fn export_extras_to_ccswitch(
    path: &Path,
    extras: &[UsageRecord],
) -> Result<UsageExportResult, DbStatus> {
    if !path.is_file() {
        return Err(DbStatus::Missing);
    }
    let extra_accounts = extra_account_count(extras);
    if extras.is_empty() {
        return Ok(UsageExportResult {
            written: 0,
            skipped: 0,
            extra_accounts,
            db_ok: true,
            message: Some(MSG_EXPORT_NONE.into()),
        });
    }
    let mut conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|err| classify_db_error(&err))?;
    conn.busy_timeout(std::time::Duration::from_millis(5_000))
        .map_err(|err| classify_db_error(&err))?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|err| classify_db_error(&err))?;
    let mut written = 0u32;
    let mut skipped = 0u32;
    {
        let mut stmt = tx.prepare(EXPORT_INSERT).map_err(|err| classify_db_error(&err))?;
        for row in extras {
            if !is_dock_extra_row(row) {
                continue;
            }
            let provider = if row.provider_name.trim().is_empty() {
                row.app_label.as_str()
            } else {
                row.provider_name.as_str()
            };
            let n = stmt
                .execute(rusqlite::params![
                    row.request_id,
                    provider,
                    row.model,
                    row.input_tokens as i64,
                    row.output_tokens as i64,
                    row.cache_read_tokens as i64,
                    row.cache_creation_tokens as i64,
                    row.input_tokens as i64,
                    cost_text(row.cost_usd),
                    row.status_code,
                    row.session_id,
                    row.created_at,
                ])
                .map_err(|err| classify_db_error(&err))?;
            if n == 0 {
                skipped += 1;
            } else {
                written += 1;
            }
        }
    }
    tx.commit().map_err(|err| classify_db_error(&err))?;
    Ok(UsageExportResult {
        written,
        skipped,
        extra_accounts,
        db_ok: true,
        message: Some(export_message(written, skipped, extras.len())),
    })
}

pub fn export_from_snapshot(snap: &UsageSnapshot) -> UsageExportResult {
    let extras: Vec<UsageRecord> = snap
        .records
        .iter()
        .filter(|row| row.data_source == "claude_extra")
        .cloned()
        .collect();
    match ccswitch_db_path() {
        None => UsageExportResult {
            written: 0,
            skipped: 0,
            extra_accounts: snap.extras.len() as u32,
            db_ok: false,
            message: db_status_message(DbStatus::Missing),
        },
        Some(path) => match export_extras_to_ccswitch(&path, &extras) {
            Ok(result) => result,
            Err(status) => UsageExportResult {
                written: 0,
                skipped: 0,
                extra_accounts: snap.extras.len() as u32,
                db_ok: false,
                message: db_status_message(status),
            },
        },
    }
}

fn read_pricing(conn: &rusqlite::Connection) -> Result<Vec<PricingRow>, DbStatus> {
    let mut stmt = conn.prepare(PRICE_SELECT).map_err(|err| classify_db_error(&err))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PricingRow {
                model_id: row.get(0)?,
                display_name: row.get(1)?,
                input_per_million: parse_price(row.get::<_, String>(2)?),
                output_per_million: parse_price(row.get::<_, String>(3)?),
                cache_read_per_million: parse_price(row.get::<_, String>(4)?),
                cache_creation_per_million: parse_price(row.get::<_, String>(5)?),
            })
        })
        .map_err(|err| classify_db_error(&err))?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn parse_price(raw: String) -> f64 {
    raw.parse().unwrap_or(0.0)
}

pub fn apply_pricing(records: &mut [UsageRecord], pricing: &[PricingRow]) {
    for record in records.iter_mut() {
        if record.data_source != "claude_extra" {
            continue;
        }
        let Some(row) = pricing.iter().find(|p| p.model_id == record.model) else {
            record.unpriced = true;
            record.cost_usd = 0.0;
            continue;
        };
        let million = 1_000_000.0;
        record.cost_usd = record.input_tokens as f64 / million * row.input_per_million
            + record.output_tokens as f64 / million * row.output_per_million
            + record.cache_read_tokens as f64 / million * row.cache_read_per_million
            + record.cache_creation_tokens as f64 / million * row.cache_creation_per_million;
        record.unpriced = false;
    }
}

pub fn app_type_label(app_type: &str) -> String {
    match app_type {
        "claude" => "Claude".into(),
        "grokbuild" => "Grok".into(),
        "opencode" => "OpenCode".into(),
        "kimi" => "Kimi".into(),
        "pi" => "Pi".into(),
        "dsh" => "DeepSeek".into(),
        "codex" => "Codex".into(),
        "gemini" => "Gemini".into(),
        other => other.to_string(),
    }
}

pub fn filter_records<'a>(records: &'a [UsageRecord], query: &UsageQuery) -> Vec<&'a UsageRecord> {
    records
        .iter()
        .filter(|row| row.created_at >= query.start && row.created_at < query.end)
        .filter(|row| query.app_filter == "all" || row.app_filter == query.app_filter)
        .filter(|row| match query.provider_name.as_deref() {
            Some(name) if !name.is_empty() => row.provider_name == name,
            _ => true,
        })
        .filter(|row| match query.model.as_deref() {
            Some(model) if !model.is_empty() => row.model == model,
            _ => true,
        })
        .collect()
}

pub fn total_tokens(row: &UsageRecord) -> u64 {
    row.input_tokens
        .saturating_add(row.output_tokens)
        .saturating_add(row.cache_read_tokens)
        .saturating_add(row.cache_creation_tokens)
}

pub fn cache_hit_percent(input: u64, cache_read: u64, cache_creation: u64) -> f64 {
    let denom = input.saturating_add(cache_read).saturating_add(cache_creation);
    if denom == 0 {
        0.0
    } else {
        cache_read as f64 / denom as f64 * 100.0
    }
}

pub fn build_apps(db_records: &[UsageRecord], extras: &[ExtraClaudeProfile]) -> Vec<UsageAppChip> {
    let mut apps = Vec::new();
    let mut seen = HashSet::new();
    for row in db_records {
        if seen.insert(row.app_filter.clone()) {
            apps.push(UsageAppChip {
                id: row.app_filter.clone(),
                label: row.app_label.clone(),
            });
        }
    }
    apps.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
    for profile in extras {
        let id = format!("claude:{}", profile.id);
        if seen.insert(id.clone()) {
            apps.push(UsageAppChip {
                id,
                label: extra_app_label(&profile.name, &profile.email, &profile.id),
            });
        }
    }
    apps
}

pub fn build_dashboard(
    records: &[UsageRecord],
    apps: Vec<UsageAppChip>,
    pricing: &[PricingRow],
    query: &UsageQuery,
    db_ok: bool,
    message: Option<String>,
) -> UsageDashboard {
    let filtered = filter_records(records, query);
    let mut input = 0u64;
    let mut output = 0u64;
    let mut cache_read = 0u64;
    let mut cache_creation = 0u64;
    let mut cost = 0.0;
    for row in &filtered {
        input = input.saturating_add(row.input_tokens);
        output = output.saturating_add(row.output_tokens);
        cache_read = cache_read.saturating_add(row.cache_read_tokens);
        cache_creation = cache_creation.saturating_add(row.cache_creation_tokens);
        cost += row.cost_usd;
    }
    let tokens = input
        .saturating_add(output)
        .saturating_add(cache_read)
        .saturating_add(cache_creation);
    UsageDashboard {
        ok: true,
        db_ok,
        message,
        total_cost_usd: cost,
        total_requests: filtered.len() as u32,
        total_tokens: tokens,
        cache_hit_percent: cache_hit_percent(input, cache_read, cache_creation),
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_creation_tokens: cache_creation,
        points: build_points(&filtered, query.start, query.end),
        heatmap: build_heatmap(&filtered, query.start, query.end),
        providers: build_groups(&filtered, |row| row.provider_name.clone()),
        models: build_groups(&filtered, |row| {
            if row.model.is_empty() {
                "(unknown)".into()
            } else {
                row.model.clone()
            }
        }),
        apps,
        pricing: pricing
            .iter()
            .map(|p| UsagePricingView {
                model_id: p.model_id.clone(),
                display_name: p.display_name.clone(),
                input_per_million: p.input_per_million,
                output_per_million: p.output_per_million,
                cache_read_per_million: p.cache_read_per_million,
                cache_creation_per_million: p.cache_creation_per_million,
            })
            .collect(),
        fetched_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn build_logs(records: &[UsageRecord], query: &UsageQuery, page: u32, page_size: u32) -> UsageLogPage {
    let mut filtered: Vec<UsageRecord> = filter_records(records, query).into_iter().cloned().collect();
    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.request_id.cmp(&b.request_id)));
    let size = page_size.max(1);
    let start = (page as u64).saturating_mul(size as u64) as usize;
    let rows = if start >= filtered.len() {
        Vec::new()
    } else {
        filtered[start..].iter().take(size as usize).cloned().collect()
    };
    UsageLogPage {
        total: filtered.len() as u32,
        page,
        page_size: size,
        rows,
    }
}

fn build_groups(rows: &[&UsageRecord], key: impl Fn(&UsageRecord) -> String) -> Vec<UsageGroupRow> {
    let mut map: std::collections::BTreeMap<String, UsageGroupRow> = std::collections::BTreeMap::new();
    for row in rows {
        let name = key(row);
        let slot = map.entry(name.clone()).or_insert_with(|| UsageGroupRow {
            name,
            request_count: 0,
            tokens: 0,
            cost_usd: 0.0,
            unpriced: false,
        });
        slot.request_count += 1;
        slot.tokens = slot.tokens.saturating_add(total_tokens(row));
        slot.cost_usd += row.cost_usd;
        slot.unpriced |= row.unpriced;
    }
    let mut out: Vec<_> = map.into_values().collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens).then(a.name.cmp(&b.name)));
    out
}

fn build_points(rows: &[&UsageRecord], start: i64, end: i64) -> Vec<UsagePoint> {
    let span = (end - start).max(1);
    let hourly = span <= HOURLY_RANGE_SECS;
    let mut map: std::collections::BTreeMap<i64, UsagePoint> = std::collections::BTreeMap::new();
    for row in rows {
        let ts = if hourly {
            row.created_at - (row.created_at.rem_euclid(3600))
        } else {
            local_midnight_ts(row.created_at)
        };
        let slot = map.entry(ts).or_insert_with(|| UsagePoint {
            ts,
            label: if hourly {
                format_hour(ts)
            } else {
                format_day(ts)
            },
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            tokens: 0,
            cost_usd: 0.0,
            requests: 0,
        });
        slot.input_tokens = slot.input_tokens.saturating_add(row.input_tokens);
        slot.output_tokens = slot.output_tokens.saturating_add(row.output_tokens);
        slot.cache_read_tokens = slot.cache_read_tokens.saturating_add(row.cache_read_tokens);
        slot.cache_creation_tokens = slot.cache_creation_tokens.saturating_add(row.cache_creation_tokens);
        slot.tokens = slot.tokens.saturating_add(total_tokens(row));
        slot.cost_usd += row.cost_usd;
        slot.requests += 1;
    }
    map.into_values().collect()
}

fn build_heatmap(rows: &[&UsageRecord], _start: i64, _end: i64) -> Vec<UsageHeatCell> {
    let mut map: std::collections::BTreeMap<i64, UsageHeatCell> = std::collections::BTreeMap::new();
    // Return occupied days. The calendar fills gaps, including ranges starting at Unix zero.
    // Advancing a local date by a fixed 86,400 seconds would also drift across DST changes.
    for row in rows {
        let ts = local_midnight_ts(row.created_at);
        let slot = map.entry(ts).or_insert_with(|| UsageHeatCell {
            ts, label: format_day(ts), tokens: 0, cost_usd: 0.0, requests: 0,
            input_tokens: 0, cache_read_tokens: 0, cache_creation_tokens: 0,
        });
        slot.tokens = slot.tokens.saturating_add(total_tokens(row));
        slot.cost_usd += row.cost_usd;
        slot.requests += 1;
        slot.input_tokens = slot.input_tokens.saturating_add(row.input_tokens);
        slot.cache_read_tokens = slot.cache_read_tokens.saturating_add(row.cache_read_tokens);
        slot.cache_creation_tokens = slot.cache_creation_tokens.saturating_add(row.cache_creation_tokens);
    }
    map.into_values().collect()
}

fn local_midnight_ts(ts: i64) -> i64 {
    use chrono::{Datelike, Local, TimeZone, Timelike};
    let dt = DateTime::from_timestamp(ts, 0)
        .map(|utc| utc.with_timezone(&Local))
        .unwrap_or_else(Local::now);
    match Local.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 0, 0, 0) {
        chrono::LocalResult::Single(midnight) | chrono::LocalResult::Ambiguous(midnight, _) => {
            midnight.timestamp()
        }
        chrono::LocalResult::None => ts - dt.hour() as i64 * 3600 - dt.minute() as i64 * 60 - dt.second() as i64,
    }
}

fn format_hour(ts: i64) -> String {
    use chrono::Local;
    DateTime::from_timestamp(ts, 0)
        .map(|utc| utc.with_timezone(&Local).format("%H:00").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn format_day(ts: i64) -> String {
    use chrono::Local;
    DateTime::from_timestamp(ts, 0)
        .map(|utc| utc.with_timezone(&Local).format("%m-%d").to_string())
        .unwrap_or_else(|| ts.to_string())
}

#[derive(Debug, Clone)]
pub struct UsageSnapshot {
    pub records: Vec<UsageRecord>,
    pub pricing: Vec<PricingRow>,
    pub status: DbStatus,
    pub files_scanned: u32,
    pub extras: Vec<ExtraClaudeProfile>,
}

static LAST_OK_DB: Mutex<Option<(Vec<UsageRecord>, Vec<PricingRow>)>> = Mutex::new(None);
static SNAPSHOT: Mutex<Option<UsageSnapshot>> = Mutex::new(None);

fn lock_mutex<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn stitch_snapshot(
    db: Result<(Vec<UsageRecord>, Vec<PricingRow>), DbStatus>,
    extra: Vec<UsageRecord>,
    files: u32,
    last_ok: &mut Option<(Vec<UsageRecord>, Vec<PricingRow>)>,
) -> (Vec<UsageRecord>, Vec<PricingRow>, DbStatus, u32) {
    match db {
        Ok((mut db_rows, pricing)) => {
            *last_ok = Some((db_rows.clone(), pricing.clone()));
            let mut extra = extra;
            apply_pricing(&mut extra, &pricing);
            db_rows.extend(extra);
            (db_rows, pricing, DbStatus::Ok, files)
        }
        Err(DbStatus::Busy) => {
            if let Some((db_rows, pricing)) = last_ok.clone() {
                let mut extra = extra;
                apply_pricing(&mut extra, &pricing);
                let mut rows = db_rows;
                rows.extend(extra);
                (rows, pricing, DbStatus::Busy, files)
            } else {
                (extra, Vec::new(), DbStatus::Busy, files)
            }
        }
        Err(status) => {
            *last_ok = None;
            (extra, Vec::new(), status, files)
        }
    }
}

pub fn load_snapshot(
    db_path: Option<&Path>,
    extras: &[ExtraClaudeProfile],
    ledger_path: Option<&Path>,
) -> (Vec<UsageRecord>, Vec<PricingRow>, DbStatus, u32) {
    let mut files = 0u32;
    for profile in extras {
        let mut found = Vec::new();
        collect_jsonl(&profile.config_dir.join("projects"), 0, &mut found);
        files += found.len() as u32;
    }
    let extra = scan_extra_profiles_filtered(extras, &load_copy_ledger(ledger_path));
    let db = match db_path {
        None => Err(DbStatus::Missing),
        Some(path) => read_ccswitch_db(path),
    };
    let mut last_ok = lock_mutex(&LAST_OK_DB);
    stitch_snapshot(db, extra, files, &mut last_ok)
}

pub fn take_snapshot(
    force: bool,
    extras: Vec<ExtraClaudeProfile>,
    ledger_path: Option<PathBuf>,
) -> UsageSnapshot {
    if !force {
        let guard = lock_mutex(&SNAPSHOT);
        if let Some(snap) = guard.as_ref() {
            return snap.clone();
        }
    }
    let (records, pricing, status, files) =
        load_snapshot(ccswitch_db_path().as_deref(), &extras, ledger_path.as_deref());
    let snap = UsageSnapshot {
        records,
        pricing,
        status,
        files_scanned: files,
        extras,
    };
    *lock_mutex(&SNAPSHOT) = Some(snap.clone());
    snap
}

pub fn dashboard_from_snapshot(snap: &UsageSnapshot, query: &UsageQuery) -> UsageDashboard {
    build_dashboard(
        &snap.records,
        build_apps(&snap.records, &snap.extras),
        &snap.pricing,
        query,
        snap.status == DbStatus::Ok,
        db_status_message(snap.status),
    )
}

pub fn sync_from_snapshot(snap: &UsageSnapshot) -> UsageSyncResult {
    UsageSyncResult {
        imported: snap.records.len() as u32,
        files_scanned: snap.files_scanned,
        extra_accounts: snap.extras.len() as u32,
        db_ok: snap.status == DbStatus::Ok,
        message: db_status_message(snap.status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn profile(dir: &Path, id: &str, name: &str) -> ExtraClaudeProfile {
        ExtraClaudeProfile {
            id: id.into(),
            name: name.into(),
            email: String::new(),
            config_dir: dir.to_path_buf(),
        }
    }

    fn write_jsonl(path: &Path, lines: &[&str]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    fn assistant(uuid: &str, ts: &str, input: u64, output: u64, cache_read: u64, cache_write: u64) -> String {
        format!(
            r#"{{"type":"assistant","uuid":"{uuid}","sessionId":"sess-1","timestamp":"{ts}","message":{{"model":"claude-opus-4","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":{cache_read},"cache_creation_input_tokens":{cache_write}}}}}}}"#
        )
    }

    #[test]
    fn extra_label_uses_name_then_email_then_short_id() {
        assert_eq!(extra_app_label("工作号", "a@b.com", "abcdef1234"), "Claude · 工作号");
        assert_eq!(extra_app_label("  ", "a@b.com", "abcdef1234"), "Claude · a@b.com");
        assert_eq!(extra_app_label("", "", "abcdef1234"), "Claude · 环境abcdef12");
    }

    #[test]
    fn sums_assistant_usage_and_skips_bad_lines() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("projects").join("p").join("sess-1.jsonl");
        write_jsonl(
            &file,
            &[
                &assistant("u1", "2026-01-17T10:30:00Z", 10, 2, 100, 20),
                "{not json",
                &assistant("u2", "2026-01-17T10:31:00Z", 3, 1, 0, 0),
            ],
        );
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert_eq!(rows.len(), 2);
        let input: u64 = rows.iter().map(|r| r.input_tokens).sum();
        assert_eq!(input, 13);
        assert_eq!(rows[0].app_filter, "claude:work");
        assert_eq!(rows[0].app_label, "Claude · 工作号");
        assert_eq!(rows[0].data_source, "claude_extra");
        assert_eq!(rows[0].status_code, 200);
        assert!(rows[0].request_id.starts_with("claude_extra:work:sess-1:"));
    }

    #[test]
    fn copy_boundary_drops_inherited_usage() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("projects").join("p").join("sess-2.jsonl");
        write_jsonl(
            &file,
            &[
                &assistant("old", "2026-01-17T10:00:00Z", 999, 9, 0, 0),
                r#"{"type":"agent-dock-copy-boundary","sessionId":"sess-2","sourceSessionId":"old-sess","copiedAt":"2026-01-17T11:00:00Z"}"#,
                &assistant("new", "2026-01-17T11:01:00Z", 4, 1, 0, 0),
            ],
        );
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, 4);
        assert!(rows[0].request_id.contains(":new"));
    }

    #[test]
    fn continue_copy_drops_inherited_usage_even_without_boundary() {
        let root = tempfile::tempdir().unwrap();
        let file = root
            .path()
            .join("projects")
            .join("p")
            .join("5b8b6dfb-7095-4a9d-80eb-69c6cb715913.jsonl");
        let now = chrono::Utc::now().to_rfc3339();
        write_jsonl(
            &file,
            &[
                &assistant("old", "2026-01-17T10:00:00Z", 999, 9, 0, 0),
                r#"{"type":"custom-title","sessionId":"5b8b6dfb-7095-4a9d-80eb-69c6cb715913","customTitle":"优化语义 · 续接"}"#,
                &assistant("new", &now, 4, 1, 0, 0),
            ],
        );
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert_eq!(rows.len(), 1, "inherited copy usage must not count after 续接");
        assert_eq!(rows[0].input_tokens, 4);
        assert!(rows[0].request_id.contains(":new"));
    }

    #[test]
    fn continue_sidecar_uses_parent_session_cut() {
        let root = tempfile::tempdir().unwrap();
        let sid = "5b8b6dfb-7095-4a9d-80eb-69c6cb715913";
        let main = root.path().join("projects").join("p").join(format!("{sid}.jsonl"));
        let agent = root
            .path()
            .join("projects")
            .join("p")
            .join(sid)
            .join("subagents")
            .join("agent-abc.jsonl");
        let now = chrono::Utc::now().to_rfc3339();
        write_jsonl(
            &main,
            &[r#"{"type":"custom-title","sessionId":"5b8b6dfb-7095-4a9d-80eb-69c6cb715913","customTitle":"优化语义 · 续接"}"#],
        );
        write_jsonl(
            &agent,
            &[
                &assistant("old", "2026-01-17T10:00:00Z", 80, 1, 0, 0),
                &assistant("new", &now, 3, 1, 0, 0),
            ],
        );
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, 3);
    }

    #[test]
    fn copy_ledger_cut_drops_inherited_without_continue_title() {
        let root = tempfile::tempdir().unwrap();
        let sid = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
        let file = root.path().join("projects").join("p").join(format!("{sid}.jsonl"));
        write_jsonl(
            &file,
            &[
                &assistant("old", "2026-01-17T10:00:00Z", 999, 9, 0, 0),
                &assistant("new", "2026-10-27T13:40:00Z", 4, 1, 0, 0),
            ],
        );
        let mut ledger = HashMap::new();
        ledger.insert(
            sid.to_string(),
            chrono::DateTime::parse_from_rfc3339("2026-10-27T13:37:00Z")
                .unwrap()
                .timestamp(),
        );
        let rows = scan_extra_profiles_filtered(&[profile(root.path(), "work", "工作号")], &ledger);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, 4);
    }

    #[test]
    fn continue_copy_inherited_usage_is_not_exported() {
        let root = tempfile::tempdir().unwrap();
        let file = root
            .path()
            .join("projects")
            .join("p")
            .join("5b8b6dfb-7095-4a9d-80eb-69c6cb715913.jsonl");
        let now = chrono::Utc::now().to_rfc3339();
        write_jsonl(
            &file,
            &[
                &assistant("old", "2026-01-17T10:00:00Z", 999, 9, 0, 0),
                r#"{"type":"custom-title","sessionId":"5b8b6dfb-7095-4a9d-80eb-69c6cb715913","customTitle":"优化语义 · 续接"}"#,
                &assistant("new", &now, 4, 1, 0, 0),
            ],
        );
        let extras = [profile(root.path(), "work", "工作号")];
        let scanned = scan_extra_profiles(&extras);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].input_tokens, 4);

        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(log_schema()).unwrap();
        drop(conn);
        let result = export_extras_to_ccswitch(&db_path, &scanned).unwrap();
        assert_eq!(result.written, 1);
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let (count, tokens): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), SUM(input_tokens) FROM proxy_request_logs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(tokens, 4);
    }

    #[test]
    fn duplicate_uuid_across_files_counts_once() {
        let root = tempfile::tempdir().unwrap();
        let main = root.path().join("projects").join("p").join("sess-1.jsonl");
        let agent = root
            .path()
            .join("projects")
            .join("p")
            .join("sess-1")
            .join("subagents")
            .join("agent-abc.jsonl");
        let line = assistant("same-id", "2026-01-17T10:30:00Z", 8, 1, 0, 0);
        write_jsonl(&main, &[&line]);
        write_jsonl(&agent, &[&line]);
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, 8);
    }

    #[test]
    fn missing_uuid_still_gets_claude_extra_request_id() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("projects").join("p").join("sess-3.jsonl");
        write_jsonl(
            &file,
            &[r#"{"type":"assistant","sessionId":"sess-3","timestamp":"2026-01-17T10:30:00Z","message":{"model":"claude-opus-4","usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#],
        );
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].request_id.starts_with("claude_extra:work:sess-3:"));
        assert_eq!(rows[0].request_id.split(':').count(), 4);
    }

    #[test]
    fn skips_dotfiles() {
        let root = tempfile::tempdir().unwrap();
        let hidden = root.path().join("projects").join("p").join(".tmp.jsonl");
        write_jsonl(
            &hidden,
            &[&assistant("h", "2026-01-17T10:30:00Z", 50, 1, 0, 0)],
        );
        let rows = scan_extra_profiles(&[profile(root.path(), "work", "工作号")]);
        assert!(rows.is_empty());
    }

    fn log_schema() -> &'static str {
        "CREATE TABLE proxy_request_logs (
            request_id TEXT PRIMARY KEY,
            created_at INTEGER NOT NULL,
            app_type TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            input_token_semantics INTEGER NOT NULL DEFAULT 0,
            total_cost_usd TEXT NOT NULL DEFAULT '0',
            session_id TEXT,
            status_code INTEGER NOT NULL,
            data_source TEXT NOT NULL DEFAULT 'proxy',
            latency_ms INTEGER NOT NULL DEFAULT 0,
            first_token_ms INTEGER
        );"
    }

    fn insert_log(
        conn: &rusqlite::Connection,
        id: &str,
        ts: i64,
        app: &str,
        model: &str,
        input: i64,
        cost: &str,
    ) {
        conn.execute(
            "INSERT INTO proxy_request_logs (
                request_id, created_at, app_type, provider_id, model,
                input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
                total_cost_usd, session_id, status_code, data_source, latency_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, 0, ?7, 's', 200, 'session', 10)",
            rusqlite::params![id, ts, app, format!("{app}-prov"), model, input, cost],
        )
        .unwrap();
    }

    #[test]
    fn db_and_extra_filters_do_not_double_count_system_claude() {
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(log_schema()).unwrap();
        insert_log(&conn, "db-claude", 1_700_000_100, "claude", "claude-opus-4", 100, "1.25");
        insert_log(&conn, "db-grok", 1_700_000_100, "grokbuild", "grok-4", 50, "0.5");
        drop(conn);

        let extra_dir = tempfile::tempdir().unwrap();
        write_jsonl(
            &extra_dir.path().join("projects").join("p").join("sess.jsonl"),
            &[&assistant("u1", "2023-11-14T22:15:00Z", 7, 1, 0, 0)],
        );
        let extras = [profile(extra_dir.path(), "work", "工作号")];
        let (records, _, status, _) = load_snapshot(Some(&db_path), &extras, None);
        assert_eq!(status, DbStatus::Ok);
        assert_eq!(records.len(), 3);

        let all = UsageQuery {
            start: 0,
            end: 2_000_000_000,
            app_filter: "all".into(),
            provider_name: None,
            model: None,
        };
        let claude = UsageQuery {
            app_filter: "claude".into(),
            ..all.clone()
        };
        let extra = UsageQuery {
            app_filter: "claude:work".into(),
            ..all.clone()
        };
        let claude_rows = filter_records(&records, &claude);
        let extra_rows = filter_records(&records, &extra);
        assert_eq!(claude_rows.len(), 1);
        assert_eq!(claude_rows[0].request_id, "db-claude");
        assert_eq!(extra_rows.len(), 1);
        assert!(extra_rows[0].request_id.starts_with("claude_extra:work:"));
        assert_eq!(filter_records(&records, &all).len(), 3);
        let ids: HashSet<_> = records.iter().map(|r| r.request_id.as_str()).collect();
        assert_eq!(ids.len(), 3);

        let dash = build_dashboard(&records, build_apps(&records, &extras), &[], &all, true, None);
        assert_eq!(dash.total_requests, 3);
        assert!(dash.apps.iter().any(|a| a.id == "claude"));
        assert!(dash.apps.iter().any(|a| a.id == "claude:work" && a.label == "Claude · 工作号"));
        assert!(dash.apps.iter().any(|a| a.id == "grokbuild"));
    }

    #[test]
    fn missing_db_still_returns_extra_accounts() {
        let extra_dir = tempfile::tempdir().unwrap();
        write_jsonl(
            &extra_dir.path().join("projects").join("p").join("sess.jsonl"),
            &[&assistant("u1", "2023-11-14T22:15:00Z", 7, 1, 0, 0)],
        );
        let extras = [profile(extra_dir.path(), "work", "工作号")];
        let missing = extra_dir.path().join("no-such.db");
        let (records, _, status, _) = load_snapshot(Some(&missing), &extras, None);
        assert_eq!(status, DbStatus::Missing);
        assert_eq!(records.len(), 1);
        let dash = build_dashboard(
            &records,
            build_apps(&records, &extras),
            &[],
            &UsageQuery {
                start: 0,
                end: 2_000_000_000,
                app_filter: "all".into(),
                provider_name: None,
                model: None,
            },
            false,
            db_status_message(status),
        );
        assert_eq!(dash.db_ok, false);
        assert_eq!(dash.message.as_deref(), Some(MSG_MISSING));
        assert_eq!(dash.total_requests, 1);
        assert!(dash.apps.iter().any(|a| a.id == "claude:work"));
    }

    #[test]
    fn schema_mismatch_when_app_type_missing() {
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE proxy_request_logs (request_id TEXT, created_at INTEGER, input_tokens INTEGER);",
        )
        .unwrap();
        drop(conn);
        let err = read_ccswitch_db(&db_path).unwrap_err();
        assert_eq!(err, DbStatus::Schema);
        assert_eq!(db_status_message(err).as_deref(), Some(MSG_SCHEMA));
    }

    #[test]
    fn extra_pricing_uses_model_table() {
        let mut rows = vec![UsageRecord {
            request_id: "claude_extra:work:s:u".into(),
            created_at: 1,
            app_filter: "claude:work".into(),
            app_label: "Claude · 工作号".into(),
            provider_name: "Claude · 工作号".into(),
            model: "claude-opus-4".into(),
            session_id: "s".into(),
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: 0.0,
            unpriced: true,
            status_code: 200,
            data_source: "claude_extra".into(),
        }];
        apply_pricing(
            &mut rows,
            &[PricingRow {
                model_id: "claude-opus-4".into(),
                display_name: "Opus".into(),
                input_per_million: 15.0,
                output_per_million: 75.0,
                cache_read_per_million: 1.5,
                cache_creation_per_million: 18.75,
            }],
        );
        assert_eq!(rows[0].unpriced, false);
        assert!((rows[0].cost_usd - 52.5).abs() < 0.0001);
    }

    #[test]
    fn cache_hit_rate_matches_spec() {
        assert_eq!(cache_hit_percent(10, 0, 0), 0.0);
        assert!((cache_hit_percent(10, 30, 10) - 60.0).abs() < 0.0001);
    }

    #[test]
    fn all_history_heatmap_keeps_recent_days_and_cache_counts() {
        let mut recent = extra_record("new", "work", 10, 0.5);
        recent.created_at = 1_793_232_000;
        recent.cache_read_tokens = 60;
        recent.cache_creation_tokens = 30;
        let mut older = extra_record("old", "work", 5, 0.1);
        older.created_at = 1_600_000_000;
        let rows = [&older, &recent, &recent];
        let heat = build_heatmap(&rows, 0, 2_000_000_000);
        assert_eq!(heat.len(), 2);
        let day = heat.iter().find(|day| day.ts == local_midnight_ts(recent.created_at)).unwrap();
        assert_eq!(day.requests, 2);
        assert_eq!(day.cache_read_tokens, 120);
        assert_eq!(day.cache_creation_tokens, 60);
        assert_eq!(day.input_tokens, 20);
        assert_eq!(build_heatmap(&[], 0, 2_000_000_000).len(), 0);
    }

    #[test]
    fn busy_keeps_previous_db_rows() {
        let db_row = UsageRecord {
            request_id: "db-claude".into(),
            created_at: 1,
            app_filter: "claude".into(),
            app_label: "Claude".into(),
            provider_name: "anthropic".into(),
            model: "opus".into(),
            session_id: "s".into(),
            input_tokens: 10,
            output_tokens: 1,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: 1.0,
            unpriced: false,
            status_code: 200,
            data_source: "session".into(),
        };
        let extra = UsageRecord {
            request_id: "claude_extra:work:s:u".into(),
            created_at: 2,
            app_filter: "claude:work".into(),
            app_label: "Claude · 工作号".into(),
            provider_name: "Claude · 工作号".into(),
            model: "opus".into(),
            session_id: "s".into(),
            input_tokens: 7,
            output_tokens: 1,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: 0.0,
            unpriced: true,
            status_code: 200,
            data_source: "claude_extra".into(),
        };
        let mut last = Some((vec![db_row.clone()], Vec::new()));
        let (records, _, status, files) =
            stitch_snapshot(Err(DbStatus::Busy), vec![extra.clone()], 3, &mut last);
        assert_eq!(status, DbStatus::Busy);
        assert_eq!(files, 3);
        assert_eq!(records.len(), 2);
        assert!(records.iter().any(|row| row.request_id == "db-claude"));
        assert!(records.iter().any(|row| row.data_source == "claude_extra"));
    }

    fn extra_record(id: &str, provider: &str, tokens: u64, cost: f64) -> UsageRecord {
        UsageRecord {
            request_id: format!("claude_extra:work:s:{id}"),
            created_at: 1_700_000_200,
            app_filter: "claude:work".into(),
            app_label: provider.into(),
            provider_name: provider.into(),
            model: "claude-opus-4".into(),
            session_id: "s".into(),
            input_tokens: tokens,
            output_tokens: 1,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: cost,
            unpriced: cost == 0.0,
            status_code: 200,
            data_source: "claude_extra".into(),
        }
    }

    #[test]
    fn export_writes_extras_as_claude_with_separate_providers() {
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(log_schema()).unwrap();
        insert_log(&conn, "db-claude", 1_700_000_100, "claude", "claude-opus-4", 100, "1.25");
        drop(conn);

        let extras = vec![
            extra_record("u1", "Claude · 工作号", 7, 0.12),
            extra_record("u2", "Claude · 副号", 3, 0.04),
        ];
        let result = export_extras_to_ccswitch(&db_path, &extras).unwrap();
        assert_eq!(result.written, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.db_ok, true);

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
        let system_cost: String = conn
            .query_row(
                "SELECT total_cost_usd FROM proxy_request_logs WHERE request_id = 'db-claude'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(system_cost, "1.25");

        let mut stmt = conn
            .prepare(
                "SELECT request_id, app_type, provider_id, data_source, input_tokens, total_cost_usd
                 FROM proxy_request_logs WHERE data_source = 'claude_extra' ORDER BY request_id",
            )
            .unwrap();
        let rows: Vec<(String, String, String, String, i64, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.1 == "claude"));
        assert!(rows.iter().all(|row| row.3 == "claude_extra"));
        let providers: HashSet<_> = rows.iter().map(|row| row.2.as_str()).collect();
        assert_eq!(providers, HashSet::from(["Claude · 工作号", "Claude · 副号"]));
        assert!(rows[0].0.starts_with("claude_extra:"));
    }

    #[test]
    fn export_is_idempotent_and_skips_existing_ids() {
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(log_schema()).unwrap();
        drop(conn);

        let extras = vec![extra_record("u1", "Claude · 工作号", 7, 0.12)];
        let first = export_extras_to_ccswitch(&db_path, &extras).unwrap();
        let second = export_extras_to_ccswitch(&db_path, &extras).unwrap();
        assert_eq!(first.written, 1);
        assert_eq!(second.written, 0);
        assert_eq!(second.skipped, 1);

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn load_snapshot_does_not_double_count_exported_extras() {
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(log_schema()).unwrap();
        insert_log(&conn, "db-claude", 1_700_000_100, "claude", "claude-opus-4", 100, "1.25");
        drop(conn);

        let extra_dir = tempfile::tempdir().unwrap();
        write_jsonl(
            &extra_dir.path().join("projects").join("p").join("sess.jsonl"),
            &[&assistant("u1", "2023-11-14T22:15:00Z", 7, 1, 0, 0)],
        );
        let extras = [profile(extra_dir.path(), "work", "工作号")];
        let scanned = scan_extra_profiles(&extras);
        export_extras_to_ccswitch(&db_path, &scanned).unwrap();

        let (records, _, status, _) = load_snapshot(Some(&db_path), &extras, None);
        assert_eq!(status, DbStatus::Ok);
        assert_eq!(records.len(), 2);
        assert_eq!(
            records.iter().filter(|row| row.data_source == "claude_extra").count(),
            1
        );
        assert_eq!(
            records.iter().filter(|row| row.app_filter == "claude").count(),
            1
        );
    }

    #[test]
    fn export_missing_db_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let err = export_extras_to_ccswitch(&dir.path().join("no.db"), &[extra_record("u1", "Claude · 工作号", 1, 0.0)])
            .unwrap_err();
        assert_eq!(err, DbStatus::Missing);
    }

    #[test]
    fn export_empty_extras_writes_nothing() {
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(log_schema()).unwrap();
        insert_log(&conn, "db-claude", 1, "claude", "opus", 1, "0");
        drop(conn);
        let result = export_extras_to_ccswitch(&db_path, &[]).unwrap();
        assert_eq!(result.written, 0);
        assert_eq!(result.skipped, 0);
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proxy_request_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
