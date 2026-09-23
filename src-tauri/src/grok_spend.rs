//! Local Grok token spend.
//!
//! Same source and accounting as CC Switch (`session_usage_grokbuild`):
//! every `updates.jsonl` under `~/.grok/{sessions,archived_sessions}`,
//! `turn_completed` only, face value per prompt (not a running snapshot).
//! Counters come from `modelUsage` when present. The same session + prompt +
//! model keeps the latest counters at the first timestamp.
//! Sessions CC Switch already imported stay in `~/.cc-switch/cc-switch.db`
//! after the log directory is deleted; those rows are kept so the total
//! matches CC Switch. `inputTokens` already includes cache read/write; the
//! total is fresh input + output + cache read + cache write.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const COST_TICK: f64 = 10_000_000_000.0;
const HOURLY_RANGE_SECS: i64 = 36 * 3600;
const MAX_COLLECT_DEPTH: usize = 16;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrokSpendPoint {
    pub ts: i64,
    pub label: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrokSpend {
    pub ok: bool,
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_hit_percent: f64,
    pub turn_count: u32,
    pub cost_usd: f64,
    pub granularity: String,
    pub range_start: i64,
    pub range_end: i64,
    pub points: Vec<GrokSpendPoint>,
    pub scanned_files: u32,
    pub fetched_at: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
struct Turn {
    ts: i64,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    cost_ticks: u64,
    /// session id + prompt id + model. Same key keeps the latest counters.
    dedup_key: String,
}

#[derive(Default)]
struct SpendCache {
    files: HashMap<String, CachedFile>,
}

struct CachedFile {
    mtime: u64,
    len: u64,
    head: u64,
    newline_terminated: bool,
    turns: Arc<Vec<Turn>>,
}

fn cache() -> std::sync::MutexGuard<'static, SpendCache> {
    static CACHE: OnceLock<Mutex<SpendCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(SpendCache::default()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn grok_home() -> PathBuf {
    std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".grok")
        })
}

pub fn fetch(start: Option<i64>, end: Option<i64>) -> GrokSpend {
    let now = Local::now();
    let range_start = start.unwrap_or_else(|| local_midnight(now.date_naive()));
    let range_end = end.unwrap_or_else(|| now.timestamp()).max(range_start + 1);
    from_home_range_with(
        &grok_home(),
        now,
        range_start,
        range_end,
        &load_ccswitch_turns(),
    )
}

#[cfg(test)]
pub fn from_home(home: &Path, now: DateTime<Local>) -> GrokSpend {
    let start = local_midnight(now.date_naive());
    let end = now
        .date_naive()
        .succ_opt()
        .map(local_midnight)
        .unwrap_or(start.saturating_add(86_400));
    from_home_range(home, now, start, end)
}

pub fn from_home_range(home: &Path, now: DateTime<Local>, start: i64, end: i64) -> GrokSpend {
    from_home_range_with(home, now, start, end, &[])
}

fn from_home_range_with(
    home: &Path,
    now: DateTime<Local>,
    start: i64,
    end: i64,
    retained: &[Turn],
) -> GrokSpend {
    let files = collect_updates_files(home);
    let mut turns = Vec::with_capacity(retained.len());
    // Retained rows go first. A turn that is still on disk replaces them when
    // the timestamp ties, so a live log and the CC Switch copy count once.
    turns.extend(retained.iter().cloned());
    let scanned = files.len() as u32;
    for path in &files {
        match load_turns(path, start) {
            Ok(items) => turns.extend(items.iter().cloned()),
            Err(_) => continue,
        }
    }
    summarize(&turns, now, start, end, scanned)
}

fn ccswitch_db_path() -> Option<PathBuf> {
    let path = dirs::home_dir()?.join(".cc-switch").join("cc-switch.db");
    path.is_file().then_some(path)
}

fn load_ccswitch_turns() -> Vec<Turn> {
    let Some(path) = ccswitch_db_path() else {
        return Vec::new();
    };
    read_ccswitch_turns(&path).unwrap_or_default()
}

fn read_ccswitch_turns(path: &Path) -> Result<Vec<Turn>, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| err.to_string())?;
    conn.busy_timeout(std::time::Duration::from_millis(2_000))
        .map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT request_id, created_at, input_tokens, output_tokens, cache_read_tokens,
                    cache_creation_tokens, total_cost_usd, input_token_semantics
             FROM proxy_request_logs
             WHERE app_type = 'grokbuild' AND data_source = 'grok_session'",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(|err| err.to_string())?;
    let mut turns = Vec::new();
    for row in rows {
        let (request_id, created_at, input, output, cache_read, cache_creation, cost, semantics) =
            row.map_err(|err| err.to_string())?;
        let Some(turn) = turn_from_ccswitch(
            &request_id,
            created_at,
            input,
            output,
            cache_read,
            cache_creation,
            &cost,
            semantics,
        ) else {
            continue;
        };
        turns.push(turn);
    }
    Ok(turns)
}

fn turn_from_ccswitch(
    request_id: &str,
    created_at: i64,
    input: i64,
    output: i64,
    cache_read: i64,
    cache_creation: i64,
    cost: &str,
    semantics: i64,
) -> Option<Turn> {
    let rest = request_id.strip_prefix("grok_session:")?;
    let mut parts = rest.splitn(3, ':');
    let session_id = parts.next().filter(|item| !item.is_empty())?;
    let prompt_key = parts.next().filter(|item| !item.is_empty())?;
    let model = parts.next().filter(|item| !item.is_empty())?;
    let input = u64::try_from(input).ok()?;
    let output = u64::try_from(output).ok()?;
    let cache_read = u64::try_from(cache_read).ok()?;
    let cache_creation = u64::try_from(cache_creation).ok()?;
    if input == 0 && output == 0 && cache_read == 0 && cache_creation == 0 {
        return None;
    }
    let counters = Counters {
        input,
        output,
        cache_read,
        cache_creation,
        cost_ticks: 0,
    };
    let fresh = if semantics == 1 {
        fresh_input(&counters)
    } else if input >= cache_read {
        input - cache_read
    } else {
        input
    };
    Some(Turn {
        ts: created_at,
        input: fresh,
        output,
        cache_read,
        cache_creation,
        cost_ticks: cost_to_ticks(cost),
        dedup_key: format!("{session_id}\u{1}{prompt_key}\u{1}{model}"),
    })
}

fn cost_to_ticks(cost: &str) -> u64 {
    let cost: f64 = cost.trim().parse().unwrap_or(0.0);
    if !cost.is_finite() || cost <= 0.0 {
        return 0;
    }
    (cost * COST_TICK).round() as u64
}

fn local_midnight(date: NaiveDate) -> i64 {
    let naive = date.and_hms_opt(0, 0, 0).unwrap_or_default();
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) | chrono::LocalResult::Ambiguous(dt, _) => dt.timestamp(),
        chrono::LocalResult::None => naive.and_utc().timestamp(),
    }
}

fn local_from_ts(ts: i64) -> DateTime<Local> {
    DateTime::from_timestamp(ts, 0)
        .map(|utc| utc.with_timezone(&Local))
        .unwrap_or_else(Local::now)
}

fn empty_turns() -> Arc<Vec<Turn>> {
    static EMPTY: OnceLock<Arc<Vec<Turn>>> = OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(Vec::new())))
}

fn collect_updates_files(home: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in ["sessions", "archived_sessions"] {
        collect_files_named(&home.join(name), "updates.jsonl", &mut files, 0);
    }
    files.sort();
    files
}

/// Recursive, symlink-skipping walk. CC Switch counts nested session logs too;
/// a fixed two-level scan misses subagent files and newer layouts.
fn collect_files_named(root: &Path, name: &str, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_COLLECT_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let path = entry.path();
        if meta.is_dir() {
            collect_files_named(&path, name, files, depth + 1);
        } else if meta.is_file() && path.file_name().and_then(|item| item.to_str()) == Some(name) {
            files.push(path);
        }
    }
}

fn load_turns(path: &Path, range_start: i64) -> Result<Arc<Vec<Turn>>, String> {
    let meta = fs::metadata(path).map_err(|err| err.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Ok(empty_turns());
    }
    let mtime = mtime_secs(&meta);
    if mtime < range_start as u64 {
        return Ok(empty_turns());
    }
    let len = meta.len();
    let key = path.to_string_lossy().to_string();
    let cached = {
        let cache = cache();
        cache.files.get(&key).map(|hit| {
            (
                hit.mtime == mtime && hit.len == len,
                hit.newline_terminated && len > hit.len,
                hit.head,
                hit.len,
                Arc::clone(&hit.turns),
            )
        })
    };
    if let Some((fresh, grew, cached_head, offset, prev)) = cached {
        if fresh {
            return Ok(prev);
        }
        if grew {
            let head = head_sig(path);
            if cached_head == head {
                if let Some(turns) = append_turns(path, &key, mtime, len, head, offset, &prev) {
                    return Ok(turns);
                }
            }
        }
    }
    let head = head_sig(path);
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    Ok(store_turns(
        &key,
        path,
        mtime,
        len,
        head,
        parse_turns_for(path, &text),
    ))
}

fn append_turns(
    path: &Path,
    key: &str,
    mtime: u64,
    len: u64,
    head: u64,
    offset: u64,
    prev: &Arc<Vec<Turn>>,
) -> Option<Arc<Vec<Turn>>> {
    let suffix = read_from(path, offset).ok()?;
    let extra = parse_turns_for(path, &suffix);
    if extra.iter().any(|turn| turn.dedup_key.contains("\u{1}idx")) {
        return None;
    }
    let mut merged = Vec::with_capacity(prev.len() + extra.len());
    merged.extend(prev.iter().cloned());
    merged.extend(extra);
    Some(store_turns(key, path, mtime, len, head, merged))
}

fn store_turns(
    key: &str,
    path: &Path,
    mtime: u64,
    len: u64,
    head: u64,
    turns: Vec<Turn>,
) -> Arc<Vec<Turn>> {
    let turns = Arc::new(turns);
    let newline_terminated = file_ends_with_newline(path, len);
    cache().files.insert(
        key.to_string(),
        CachedFile {
            mtime,
            len,
            head,
            newline_terminated,
            turns: Arc::clone(&turns),
        },
    );
    turns
}

fn session_id_of(path: &Path) -> String {
    path.parent()
        .and_then(|dir| dir.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_turns_for(path: &Path, content: &str) -> Vec<Turn> {
    parse_session_turns(content, &session_id_of(path))
}

fn head_sig(path: &Path) -> u64 {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return 0,
    };
    let mut buf = [0u8; 256];
    let n = file.read(&mut buf).unwrap_or(0);
    let mut hash = 0xcbf29ce484222325u64;
    for byte in &buf[..n] {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn read_from(path: &Path, offset: u64) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|err| err.to_string())?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|err| err.to_string())?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .map_err(|err| err.to_string())?;
    Ok(buf)
}

fn file_ends_with_newline(path: &Path, len: u64) -> bool {
    if len == 0 {
        return true;
    }
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    if file.seek(SeekFrom::End(-1)).is_err() {
        return false;
    }
    let mut byte = [0u8; 1];
    file.read_exact(&mut byte).is_ok() && byte[0] == b'\n'
}

fn mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_turns(content: &str) -> Vec<Turn> {
    parse_session_turns(content, "sess")
}

fn parse_session_turns(content: &str, session_id: &str) -> Vec<Turn> {
    let mut out = Vec::new();
    let mut event_index = 0u32;
    for line in content.lines() {
        if !line.contains("turn_completed") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(turns) = parse_turn_models(&value, session_id, event_index) else {
            continue;
        };
        if turns.is_empty() {
            continue;
        }
        event_index = event_index.saturating_add(1);
        out.extend(turns);
    }
    out
}

fn parse_turn_models(value: &Value, session_id: &str, event_index: u32) -> Option<Vec<Turn>> {
    if value.get("method").and_then(Value::as_str) != Some("_x.ai/session/update") {
        return None;
    }
    let update = value.get("params")?.get("update")?;
    let kind = update.get("sessionUpdate").and_then(Value::as_str);
    if kind.is_some() && kind != Some("turn_completed") {
        return None;
    }
    let usage = update.get("usage")?;
    if !usage.is_object() {
        return None;
    }
    let ts = parse_timestamp(value.get("timestamp"))?;
    let prompt_id = update
        .get("prompt_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let prompt_key = if prompt_id.is_empty() {
        format!("idx{event_index}")
    } else {
        prompt_id.to_string()
    };
    let top = counters_from(usage);
    let mut models: Vec<(String, Counters)> = usage
        .get("modelUsage")
        .and_then(Value::as_object)
        .map(|map| {
            let mut rows: Vec<(String, Counters)> = map
                .iter()
                .map(|(model, counters)| (model.clone(), counters_from(counters)))
                .collect();
            rows.sort_by(|left, right| left.0.cmp(&right.0));
            rows
        })
        .unwrap_or_default();
    if models.is_empty() {
        models.push(("unknown".to_string(), top));
    } else if models.iter().all(|(_, counters)| counters.cost_ticks == 0) && top.cost_ticks > 0 {
        models[0].1.cost_ticks = top.cost_ticks;
    }
    let mut turns = Vec::new();
    for (model, counters) in models {
        if counters.is_zero() {
            continue;
        }
        turns.push(Turn {
            ts,
            input: fresh_input(&counters),
            output: counters.output,
            cache_read: counters.cache_read,
            cache_creation: counters.cache_creation,
            cost_ticks: counters.cost_ticks,
            dedup_key: format!("{session_id}\u{1}{prompt_key}\u{1}{model}"),
        });
    }
    Some(turns)
}

struct Counters {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    cost_ticks: u64,
}

impl Counters {
    fn is_zero(&self) -> bool {
        self.input == 0 && self.output == 0 && self.cache_read == 0 && self.cache_creation == 0
    }
}

/// CC Switch stores raw input and subtracts cache only when input covers it
/// (`input_token_semantics = TOTAL`).
fn fresh_input(counters: &Counters) -> u64 {
    let covered = counters.cache_read.saturating_add(counters.cache_creation);
    if counters.input >= covered {
        counters.input - covered
    } else if counters.input >= counters.cache_read {
        counters.input - counters.cache_read
    } else {
        counters.input
    }
}

fn counters_from(value: &Value) -> Counters {
    let get = |key: &str| {
        let field = match value.get(key) {
            Some(item) => item,
            None => return 0,
        };
        field
            .as_u64()
            .or_else(|| field.as_i64().and_then(|n| u64::try_from(n).ok()))
            .or_else(|| {
                field.as_f64().and_then(|n| {
                    if n.is_finite() && n >= 0.0 {
                        Some(n as u64)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(0)
    };
    let cache_read = if value.get("cachedReadTokens").is_some() {
        get("cachedReadTokens")
    } else {
        get("cacheReadTokens")
    };
    let cache_creation = if value.get("cacheCreationTokens").is_some() {
        get("cacheCreationTokens")
    } else {
        get("cacheWriteTokens")
    };
    Counters {
        input: get("inputTokens"),
        output: get("outputTokens"),
        cache_read,
        cache_creation,
        cost_ticks: get("costUsdTicks"),
    }
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    if let Some(n) = value.as_u64() {
        let n = n as i64;
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    value
        .as_str()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
}

fn build_slots(start: i64, end: i64) -> (String, Vec<(i64, String)>) {
    if end.saturating_sub(start) <= HOURLY_RANGE_SECS {
        ("hour".into(), hour_slots(start, end))
    } else {
        ("day".into(), day_slots(start, end))
    }
}

fn hour_slots(start: i64, end: i64) -> Vec<(i64, String)> {
    let mut dt = local_from_ts(start);
    dt = dt
        .with_minute(0)
        .and_then(|item| item.with_second(0))
        .and_then(|item| item.with_nanosecond(0))
        .unwrap_or(dt);
    let mut slots = Vec::new();
    while dt.timestamp() < end {
        slots.push((
            dt.timestamp(),
            format!("{}/{} {:02}:00", dt.month(), dt.day(), dt.hour()),
        ));
        dt = match dt.checked_add_signed(Duration::hours(1)) {
            Some(next) => next,
            None => break,
        };
        if slots.len() > 48 {
            break;
        }
    }
    if slots.is_empty() {
        let dt = local_from_ts(start);
        slots.push((
            start,
            format!("{}/{} {:02}:00", dt.month(), dt.day(), dt.hour()),
        ));
    }
    slots
}

fn day_slots(start: i64, end: i64) -> Vec<(i64, String)> {
    let mut date = local_from_ts(start).date_naive();
    let last = local_from_ts(end.saturating_sub(1).max(start)).date_naive();
    let mut slots = Vec::new();
    while date <= last {
        slots.push((
            local_midnight(date),
            format!("{}/{}", date.month(), date.day()),
        ));
        match date.succ_opt() {
            Some(next) => date = next,
            None => break,
        }
        if slots.len() > 62 {
            break;
        }
    }
    if slots.is_empty() {
        slots.push((start, String::new()));
    }
    slots
}

fn slot_index(slots: &[(i64, String)], ts: i64) -> Option<usize> {
    match slots.binary_search_by(|slot| slot.0.cmp(&ts)) {
        Ok(index) => Some(index),
        Err(index) if index > 0 => Some(index - 1),
        _ => None,
    }
}

fn cache_hit_percent(input: u64, cache_read: u64, cache_creation: u64) -> f64 {
    let denom = input
        .saturating_add(cache_read)
        .saturating_add(cache_creation);
    if denom == 0 {
        0.0
    } else {
        (cache_read as f64 / denom as f64) * 100.0
    }
}

struct Deduped {
    first_ts: i64,
    latest_ts: i64,
    turn: Turn,
}

/// Latest counters stay on the first timestamp, matching CC Switch's upsert:
/// a rewritten `prompt_id` replaces the row instead of adding another one.
fn dedup_turns(turns: &[Turn]) -> Vec<Turn> {
    let mut order = Vec::new();
    let mut map: HashMap<String, Deduped> = HashMap::new();
    for turn in turns {
        if let Some(slot) = map.get_mut(&turn.dedup_key) {
            if turn.ts < slot.first_ts {
                slot.first_ts = turn.ts;
            }
            if turn.ts >= slot.latest_ts {
                slot.latest_ts = turn.ts;
                slot.turn = turn.clone();
            }
            slot.turn.ts = slot.first_ts;
            continue;
        }
        order.push(turn.dedup_key.clone());
        map.insert(
            turn.dedup_key.clone(),
            Deduped {
                first_ts: turn.ts,
                latest_ts: turn.ts,
                turn: turn.clone(),
            },
        );
    }
    order
        .into_iter()
        .filter_map(|key| map.remove(&key))
        .map(|slot| slot.turn)
        .collect()
}

fn summarize(
    turns: &[Turn],
    now: DateTime<Local>,
    start: i64,
    end: i64,
    scanned: u32,
) -> GrokSpend {
    let turns = dedup_turns(turns);
    let (granularity, slots) = build_slots(start, end);
    let mut points: Vec<GrokSpendPoint> = slots
        .iter()
        .map(|(ts, label)| GrokSpendPoint {
            ts: *ts,
            label: label.clone(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: 0.0,
        })
        .collect();
    let mut input = 0u64;
    let mut output = 0u64;
    let mut cache_read = 0u64;
    let mut cache_creation = 0u64;
    let mut cost_ticks = 0u64;
    let mut turn_count = 0u32;
    for turn in turns {
        if turn.ts < start || turn.ts >= end {
            continue;
        }
        let Some(index) = slot_index(&slots, turn.ts) else {
            continue;
        };
        let slot = &mut points[index];
        slot.input_tokens = slot.input_tokens.saturating_add(turn.input);
        slot.output_tokens = slot.output_tokens.saturating_add(turn.output);
        slot.cache_read_tokens = slot.cache_read_tokens.saturating_add(turn.cache_read);
        slot.cache_creation_tokens = slot
            .cache_creation_tokens
            .saturating_add(turn.cache_creation);
        slot.cost_usd += turn.cost_ticks as f64 / COST_TICK;
        input = input.saturating_add(turn.input);
        output = output.saturating_add(turn.output);
        cache_read = cache_read.saturating_add(turn.cache_read);
        cache_creation = cache_creation.saturating_add(turn.cache_creation);
        cost_ticks = cost_ticks.saturating_add(turn.cost_ticks);
        turn_count = turn_count.saturating_add(1);
    }
    let total = input
        .saturating_add(output)
        .saturating_add(cache_read)
        .saturating_add(cache_creation);
    GrokSpend {
        ok: true,
        total_tokens: total,
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_creation_tokens: cache_creation,
        cache_hit_percent: cache_hit_percent(input, cache_read, cache_creation),
        turn_count,
        cost_usd: cost_ticks as f64 / COST_TICK,
        granularity,
        range_start: start,
        range_end: end,
        points,
        scanned_files: scanned,
        fetched_at: now.to_rfc3339(),
        message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn turn_line(ts: i64, prompt: &str, input: u64, output: u64, cache: u64, ticks: u64) -> String {
        serde_json::json!({
            "timestamp": ts,
            "method": "_x.ai/session/update",
            "params": {
                "sessionId": "s1",
                "update": {
                    "sessionUpdate": "turn_completed",
                    "prompt_id": prompt,
                    "stop_reason": "end_turn",
                    "usage": {
                        "inputTokens": input,
                        "outputTokens": output,
                        "totalTokens": input + output,
                        "cachedReadTokens": cache,
                        "cacheCreationTokens": 0,
                        "modelCalls": 1,
                        "costUsdTicks": ticks,
                        "modelUsage": {
                            "grok-4.6-build": {
                                "inputTokens": input,
                                "outputTokens": output,
                                "cachedReadTokens": cache,
                                "costUsdTicks": ticks
                            }
                        }
                    }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn parses_face_value_turns_and_splits_uncached_input() {
        let t1 = turn_line(1_700_000_000, "p1", 17_294, 28, 11_136, 158_248_000);
        let t2 = turn_line(1_700_000_060, "p2", 17_347, 56, 17_280, 56_540_000);
        let turns = parse_turns(&format!("{t1}\n{t2}\n"));
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].input, 17_294 - 11_136);
        assert_eq!(turns[0].cache_read, 11_136);
        assert_eq!(turns[0].output, 28);
        assert_eq!(turns[1].input, 17_347 - 17_280);
        assert_eq!(turns[1].output, 56);
    }

    #[test]
    fn ignores_non_completed_and_broken_lines() {
        let content = concat!(
            "{\"timestamp\":100,\"method\":\"session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"agent_message_chunk\"}}}\n",
            "not json\n",
            "{\"timestamp\":200,\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"usage_snapshot\",\"usage\":{\"inputTokens\":9,\"outputTokens\":1}}}}\n",
        );
        assert!(parse_turns(content).is_empty());
    }

    #[test]
    fn two_identical_turns_both_count() {
        let now = Local.with_ymd_and_hms(2026, 9, 11, 19, 0, 0).unwrap();
        let start = local_midnight(now.date_naive());
        let ts = start + 10 * 3600;
        let first = turn_line(ts, "p1", 100, 10, 0, 0);
        let second = turn_line(ts + 1, "p2", 100, 10, 0, 0);
        let turns = parse_turns(&format!("{first}\n{second}\n"));
        let spend = summarize(&turns, now, start, start + 86_400, 1);
        assert_eq!(spend.turn_count, 2);
        assert_eq!(spend.input_tokens, 200);
        assert_eq!(spend.output_tokens, 20);
        assert_eq!(spend.total_tokens, 220);
        assert_eq!(spend.granularity, "hour");
        assert_eq!(spend.points[10].input_tokens, 200);
    }

    #[test]
    fn repeated_prompt_keeps_latest_counters_on_the_first_timestamp() {
        let now = Local.with_ymd_and_hms(2026, 9, 11, 19, 0, 0).unwrap();
        let start = local_midnight(now.date_naive());
        let ts = start + 10 * 3600;
        let first = turn_line(ts, "p1", 100, 10, 0, 0);
        let second = turn_line(ts + 120, "p1", 500, 40, 100, 0);
        let turns = parse_turns(&format!("{first}\n{second}\n"));
        assert_eq!(turns.len(), 2);
        let spend = summarize(&turns, now, start, start + 86_400, 1);
        assert_eq!(spend.turn_count, 1);
        assert_eq!(spend.input_tokens, 400);
        assert_eq!(spend.output_tokens, 40);
        assert_eq!(spend.cache_read_tokens, 100);
        assert_eq!(spend.points[10].output_tokens, 40);
        assert_eq!(spend.points[10].input_tokens, 400);
    }

    #[test]
    fn model_usage_wins_over_cumulative_top_level() {
        let line = r#"{"timestamp":1700000000,"method":"_x.ai/session/update","params":{"update":{"sessionUpdate":"turn_completed","prompt_id":"p9","usage":{"inputTokens":900000,"outputTokens":800000,"cachedReadTokens":0,"costUsdTicks":1,"modelUsage":{"grok-4.6":{"inputTokens":1000,"outputTokens":20,"cachedReadTokens":400,"costUsdTicks":50}}}}}}"#;
        let turns = parse_turns(line);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].input, 600);
        assert_eq!(turns[0].output, 20);
        assert_eq!(turns[0].cache_read, 400);
        assert_eq!(turns[0].cost_ticks, 50);
    }

    #[test]
    fn real_total_is_uncached_plus_output_plus_cache() {
        let now = Local.with_ymd_and_hms(2026, 9, 11, 12, 0, 0).unwrap();
        let start = local_midnight(now.date_naive());
        let ts = start + 11 * 3600;
        let turns = parse_turns(&turn_line(
            ts,
            "p1",
            8_384_747,
            52_754,
            8_013_952,
            8_610_653_000,
        ));
        let spend = summarize(&turns, now, start, start + 86_400, 1);
        assert_eq!(spend.input_tokens, 8_384_747 - 8_013_952);
        assert_eq!(spend.output_tokens, 52_754);
        assert_eq!(spend.cache_read_tokens, 8_013_952);
        assert_eq!(
            spend.total_tokens,
            spend.input_tokens + spend.output_tokens + spend.cache_read_tokens
        );
        assert!((spend.cost_usd - 0.8610653).abs() < 1e-9);
        let expected = 8_013_952.0 / (spend.input_tokens + spend.cache_read_tokens) as f64 * 100.0;
        assert!((spend.cache_hit_percent - expected).abs() < 1e-9);
    }

    #[test]
    fn week_range_uses_daily_buckets() {
        let now = Local.with_ymd_and_hms(2026, 9, 11, 19, 0, 0).unwrap();
        let end = now.timestamp();
        let start = local_midnight(now.date_naive()) - 6 * 86_400;
        let day0 = local_midnight(now.date_naive());
        let day1 = day0 - 86_400;
        let turns = parse_turns(&format!(
            "{}\n{}\n",
            turn_line(day1 + 3600, "p1", 200, 10, 50, 0),
            turn_line(day0 + 3600, "p2", 400, 20, 100, 0)
        ));
        let spend = summarize(&turns, now, start, end, 1);
        assert_eq!(spend.granularity, "day");
        assert_eq!(spend.points.len(), 7);
        assert_eq!(spend.turn_count, 2);
        assert_eq!(spend.input_tokens, 450);
        assert_eq!(spend.output_tokens, 30);
    }

    #[test]
    fn from_home_reads_session_layout_including_nested_logs() {
        let dir = tempfile::tempdir().unwrap();
        let sess = dir
            .path()
            .join("sessions")
            .join("enc-project")
            .join("01abc");
        fs::create_dir_all(&sess).unwrap();
        let nested = sess.join("subagent").join("child");
        fs::create_dir_all(&nested).unwrap();
        let now = Local::now();
        let start = local_midnight(now.date_naive());
        let ts = start + 15 * 3600;
        let mut file = fs::File::create(sess.join("updates.jsonl")).unwrap();
        writeln!(file, "{}", turn_line(ts, "p1", 150, 20, 50, 0)).unwrap();
        let mut nested_file = fs::File::create(nested.join("updates.jsonl")).unwrap();
        writeln!(nested_file, "{}", turn_line(ts, "p2", 9999, 9, 0, 0)).unwrap();

        let spend = from_home(dir.path(), now);
        assert_eq!(spend.scanned_files, 2);
        assert_eq!(spend.turn_count, 2);
        assert_eq!(spend.input_tokens, 10_099);
        assert_eq!(spend.output_tokens, 29);
        assert_eq!(spend.cache_read_tokens, 50);
        assert_eq!(spend.total_tokens, 10_178);
        let hour15 = spend.points.iter().find(|p| p.label.contains("15:00"));
        assert_eq!(
            hour15.map(|p| p.input_tokens + p.output_tokens + p.cache_read_tokens),
            Some(10_178)
        );
    }

    #[test]
    fn archived_copy_of_the_same_session_is_not_double_counted() {
        let dir = tempfile::tempdir().unwrap();
        let now = Local::now();
        let start = local_midnight(now.date_naive());
        let ts = start + 12 * 3600;
        for root in ["sessions", "archived_sessions"] {
            let sess = dir.path().join(root).join("enc").join("same-session");
            fs::create_dir_all(&sess).unwrap();
            let mut file = fs::File::create(sess.join("updates.jsonl")).unwrap();
            writeln!(file, "{}", turn_line(ts, "p1", 150, 20, 50, 0)).unwrap();
        }
        let spend = from_home(dir.path(), now);
        assert_eq!(spend.scanned_files, 2);
        assert_eq!(spend.turn_count, 1);
        assert_eq!(spend.input_tokens, 100);
        assert_eq!(spend.output_tokens, 20);
        assert_eq!(spend.cache_read_tokens, 50);
    }

    #[test]
    fn appending_a_session_file_does_not_reread_or_double_count() {
        let dir = tempfile::tempdir().unwrap();
        let sess = dir.path().join("sessions").join("enc").join("live");
        fs::create_dir_all(&sess).unwrap();
        let path = sess.join("updates.jsonl");
        let now = Local::now();
        let start = local_midnight(now.date_naive());
        let ts = start + 9 * 3600;
        fs::write(&path, format!("{}\n", turn_line(ts, "p1", 100, 10, 0, 0))).unwrap();
        let first = from_home(dir.path(), now);
        assert_eq!(first.turn_count, 1);
        assert_eq!(first.output_tokens, 10);

        use std::io::Write as _;
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "{}", turn_line(ts + 30, "p2", 80, 5, 0, 0)).unwrap();
        writeln!(file, "{}", turn_line(ts + 40, "p1", 100, 25, 0, 0)).unwrap();
        file.flush().unwrap();

        let second = from_home(dir.path(), now);
        assert_eq!(second.turn_count, 2);
        assert_eq!(second.output_tokens, 30);
        assert_eq!(second.input_tokens, 180);
    }

    #[test]
    fn stale_files_are_not_read_for_today() {
        let dir = tempfile::tempdir().unwrap();
        let sess = dir.path().join("sessions").join("enc").join("old");
        fs::create_dir_all(&sess).unwrap();
        let path = sess.join("updates.jsonl");
        fs::write(&path, turn_line(1_700_000_000, "p1", 100, 10, 0, 0)).unwrap();
        let yesterday = std::time::SystemTime::now() - std::time::Duration::from_secs(86_400 * 2);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(yesterday)
            .unwrap();
        let spend = from_home(dir.path(), Local::now());
        assert_eq!(spend.turn_count, 0);
        assert_eq!(spend.total_tokens, 0);
    }

    #[test]
    fn ccswitch_ledger_keeps_deleted_sessions_once() {
        let dir = tempfile::tempdir().unwrap();
        let sess = dir.path().join("sessions").join("live");
        fs::create_dir_all(&sess).unwrap();
        let now = Local::now();
        let start = local_midnight(now.date_naive());
        let ts = start + 9 * 3600;
        fs::write(
            sess.join("updates.jsonl"),
            format!("{}\n", turn_line(ts, "p1", 1000, 10, 400, 0)),
        )
        .unwrap();

        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("cc-switch.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE proxy_request_logs (
                request_id TEXT,
                created_at INTEGER,
                input_tokens INTEGER,
                output_tokens INTEGER,
                cache_read_tokens INTEGER,
                cache_creation_tokens INTEGER,
                total_cost_usd TEXT,
                input_token_semantics INTEGER,
                app_type TEXT,
                data_source TEXT
            );",
        )
        .unwrap();
        let insert =
            "INSERT INTO proxy_request_logs VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
        conn.execute(
            insert,
            rusqlite::params![
                "grok_session:live:p1:grok-4.6-build",
                ts,
                1000_i64,
                10_i64,
                400_i64,
                0_i64,
                "1.5",
                1_i64,
                "grokbuild",
                "grok_session"
            ],
        )
        .unwrap();
        conn.execute(
            insert,
            rusqlite::params![
                "grok_session:gone:p2:grok-4.6-build",
                ts,
                2000_i64,
                20_i64,
                500_i64,
                0_i64,
                "2.25",
                1_i64,
                "grokbuild",
                "grok_session"
            ],
        )
        .unwrap();
        conn.execute(
            insert,
            rusqlite::params![
                "grok_session:other:p3:grok-4.6-build",
                ts,
                9_i64,
                9_i64,
                0_i64,
                0_i64,
                "9",
                1_i64,
                "codex",
                "grok_session"
            ],
        )
        .unwrap();
        conn.execute(
            insert,
            rusqlite::params![
                "not-a-grok-id",
                ts,
                9_i64,
                9_i64,
                0_i64,
                0_i64,
                "9",
                1_i64,
                "grokbuild",
                "grok_session"
            ],
        )
        .unwrap();
        drop(conn);

        let retained = read_ccswitch_turns(&db_path).unwrap();
        assert_eq!(retained.len(), 2);
        let spend = from_home_range_with(dir.path(), now, start, start + 86_400, &retained);
        assert_eq!(spend.turn_count, 2);
        assert_eq!(spend.input_tokens, 2_100);
        assert_eq!(spend.output_tokens, 30);
        assert_eq!(spend.cache_read_tokens, 900);
        assert_eq!(spend.total_tokens, 3_030);
        assert!((spend.cost_usd - 2.25).abs() < 1e-9);
        assert!(turn_from_ccswitch("grok_session:only-two", ts, 1, 1, 0, 0, "1", 1).is_none());
    }
}
