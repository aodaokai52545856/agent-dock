//! Local Grok token spend.
//!
//! Same source as CC Switch: `~/.grok/{sessions,archived_sessions}/<cwd>/<id>/updates.jsonl`
//! `turn_completed` usage, counted at face value (not a running snapshot).
//! Grok `inputTokens` already includes cache reads; the status line uses the
//! cache-normalized total: uncached input + output + cache read + cache write.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const COST_TICK: f64 = 10_000_000_000.0;
const HOURLY_RANGE_SECS: i64 = 36 * 3600;

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
}

#[derive(Default)]
struct SpendCache {
    files: HashMap<String, CachedFile>,
}

struct CachedFile {
    mtime: u64,
    turns: Vec<Turn>,
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
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".grok"))
}

pub fn fetch(start: Option<i64>, end: Option<i64>) -> GrokSpend {
    let now = Local::now();
    let range_start = start.unwrap_or_else(|| local_midnight(now.date_naive()));
    let range_end = end.unwrap_or_else(|| now.timestamp()).max(range_start + 1);
    from_home_range(&grok_home(), now, range_start, range_end)
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
    let files = collect_updates_files(home);
    let mut turns = Vec::new();
    let scanned = files.len() as u32;
    for path in &files {
        match load_turns(path, start) {
            Ok(items) => turns.extend(items),
            Err(_) => continue,
        }
    }
    summarize(&turns, now, start, end, scanned)
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

fn collect_updates_files(home: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in ["sessions", "archived_sessions"] {
        collect_session_updates(&home.join(name), &mut files);
    }
    files
}

fn collect_session_updates(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(cwd_dirs) = fs::read_dir(root) else {
        return;
    };
    for cwd in cwd_dirs.flatten() {
        let cwd_path = cwd.path();
        if !is_real_dir(&cwd_path) {
            continue;
        }
        let Ok(sessions) = fs::read_dir(&cwd_path) else {
            continue;
        };
        for session in sessions.flatten() {
            let session_path = session.path();
            if !is_real_dir(&session_path) {
                continue;
            }
            let updates = session_path.join("updates.jsonl");
            if is_real_file(&updates) {
                files.push(updates);
            }
        }
    }
}

fn is_real_dir(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.is_dir() && !meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

fn is_real_file(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.is_file() && !meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

fn load_turns(path: &Path, range_start: i64) -> Result<Vec<Turn>, String> {
    let meta = fs::metadata(path).map_err(|err| err.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Ok(Vec::new());
    }
    let mtime = mtime_secs(&meta);
    if mtime < range_start as u64 {
        return Ok(Vec::new());
    }
    let key = path.to_string_lossy().to_string();
    {
        let cache = cache();
        if let Some(hit) = cache.files.get(&key) {
            if hit.mtime == mtime {
                return Ok(hit.turns.clone());
            }
        }
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let turns = parse_turns(&text);
    cache().files.insert(key, CachedFile { mtime, turns: turns.clone() });
    Ok(turns)
}

fn mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_turns(content: &str) -> Vec<Turn> {
    let mut out = Vec::new();
    for line in content.lines() {
        if !line.contains("turn_completed") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(turn) = parse_turn(&value) {
            out.push(turn);
        }
    }
    out
}

fn parse_turn(value: &Value) -> Option<Turn> {
    let update = value.get("params")?.get("update")?;
    let kind = update.get("sessionUpdate").and_then(Value::as_str);
    if kind.is_some() && kind != Some("turn_completed") {
        return None;
    }
    let usage = update.get("usage")?;
    let counters = parse_counters(usage)?;
    if counters.input == 0 && counters.output == 0 && counters.cache_read == 0 && counters.cache_creation == 0 {
        return None;
    }
    let ts = parse_timestamp(value.get("timestamp"))?;
    Some(Turn {
        ts,
        input: counters.input.saturating_sub(counters.cache_read),
        output: counters.output,
        cache_read: counters.cache_read,
        cache_creation: counters.cache_creation,
        cost_ticks: counters.cost_ticks,
    })
}

struct Counters {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    cost_ticks: u64,
}

fn parse_counters(usage: &Value) -> Option<Counters> {
    if usage.get("inputTokens").is_some() || usage.get("outputTokens").is_some() {
        return Some(counters_from(usage));
    }
    let map = usage.get("modelUsage")?.as_object()?;
    if map.is_empty() {
        return None;
    }
    let mut total = Counters {
        input: 0,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
        cost_ticks: 0,
    };
    for item in map.values() {
        let one = counters_from(item);
        total.input = total.input.saturating_add(one.input);
        total.output = total.output.saturating_add(one.output);
        total.cache_read = total.cache_read.saturating_add(one.cache_read);
        total.cache_creation = total.cache_creation.saturating_add(one.cache_creation);
        total.cost_ticks = total.cost_ticks.saturating_add(one.cost_ticks);
    }
    Some(total)
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
    Counters {
        input: get("inputTokens"),
        output: get("outputTokens"),
        cache_read: get("cachedReadTokens"),
        cache_creation: get("cacheCreationTokens"),
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
        slots.push((start, format!("{}/{} {:02}:00", dt.month(), dt.day(), dt.hour())));
    }
    slots
}

fn day_slots(start: i64, end: i64) -> Vec<(i64, String)> {
    let mut date = local_from_ts(start).date_naive();
    let last = local_from_ts(end.saturating_sub(1).max(start)).date_naive();
    let mut slots = Vec::new();
    while date <= last {
        slots.push((local_midnight(date), format!("{}/{}", date.month(), date.day())));
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
    let denom = input.saturating_add(cache_read).saturating_add(cache_creation);
    if denom == 0 {
        0.0
    } else {
        (cache_read as f64 / denom as f64) * 100.0
    }
}

fn summarize(turns: &[Turn], now: DateTime<Local>, start: i64, end: i64, scanned: u32) -> GrokSpend {
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
        slot.cache_creation_tokens = slot.cache_creation_tokens.saturating_add(turn.cache_creation);
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
        let line = turn_line(ts, "p1", 100, 10, 0, 0);
        let turns = parse_turns(&format!("{line}\n{line}\n"));
        let spend = summarize(&turns, now, start, start + 86_400, 1);
        assert_eq!(spend.turn_count, 2);
        assert_eq!(spend.input_tokens, 200);
        assert_eq!(spend.output_tokens, 20);
        assert_eq!(spend.total_tokens, 220);
        assert_eq!(spend.granularity, "hour");
        assert_eq!(spend.points[10].input_tokens, 200);
    }

    #[test]
    fn real_total_is_uncached_plus_output_plus_cache() {
        let now = Local.with_ymd_and_hms(2026, 9, 11, 12, 0, 0).unwrap();
        let start = local_midnight(now.date_naive());
        let ts = start + 11 * 3600;
        let turns = parse_turns(&turn_line(ts, "p1", 8_384_747, 52_754, 8_013_952, 8_610_653_000));
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
    fn from_home_reads_session_layout_and_skips_nested_subagents() {
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
        assert_eq!(spend.scanned_files, 1);
        assert_eq!(spend.turn_count, 1);
        assert_eq!(spend.input_tokens, 100);
        assert_eq!(spend.output_tokens, 20);
        assert_eq!(spend.cache_read_tokens, 50);
        assert_eq!(spend.total_tokens, 170);
        let hour15 = spend.points.iter().find(|p| p.label.contains("15:00"));
        assert_eq!(hour15.map(|p| p.input_tokens + p.output_tokens + p.cache_read_tokens), Some(170));
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
}
