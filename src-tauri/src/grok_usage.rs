use crate::proxy::validate_proxy_url;
use crate::state::{self, AppSettings, Project};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tauri::AppHandle;

const DEFAULT_PROXY_BASE: &str = "https://cli-chat-proxy.grok.com/v1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrokUsage {
    pub ok: bool,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub resets_at: Option<String>,
    pub period_label: Option<String>,
    pub prepaid_balance: Option<f64>,
    pub on_demand_used: Option<f64>,
    pub on_demand_cap: Option<f64>,
    pub grok_build_used_percent: Option<f64>,
    pub used_credits: Option<f64>,
    pub credit_limit: Option<f64>,
    pub fetched_at: String,
    pub message: Option<String>,
}

impl GrokUsage {
    fn fail(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            used_percent: None,
            remaining_percent: None,
            resets_at: None,
            period_label: None,
            prepaid_balance: None,
            on_demand_used: None,
            on_demand_cap: None,
            grok_build_used_percent: None,
            used_credits: None,
            credit_limit: None,
            fetched_at: Utc::now().to_rfc3339(),
            message: Some(message.into()),
        }
    }
}

pub fn fetch(app: &AppHandle, project_id: Option<&str>) -> GrokUsage {
    let token = match read_session_token() {
        Ok(token) => token,
        Err(message) => return GrokUsage::fail(message),
    };
    let state = match state::load_state(app) {
        Ok(state) => state,
        Err(message) => return GrokUsage::fail(message),
    };
    let project = project_id.and_then(|id| state::find_project(&state, id).ok());
    match fetch_billing(&token, project, &state.settings) {
        Ok(body) => match parse_billing(&body) {
            Ok(usage) => usage,
            Err(message) => GrokUsage::fail(message),
        },
        Err(message) => GrokUsage::fail(message),
    }
}

fn grok_home() -> PathBuf {
    std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".grok"))
}

fn read_session_token() -> Result<String, String> {
    let path = grok_home().join("auth.json");
    if !path.is_file() {
        return Err("没有找到 Grok 登录，请先 grok login。".into());
    }
    let text = fs::read_to_string(&path).map_err(|err| format!("读取 Grok 登录失败：{err}"))?;
    let value: Value = serde_json::from_str(&text).map_err(|_| "Grok 登录文件损坏，请重新登录。".to_string())?;
    find_session_key(&value).ok_or_else(|| "Grok 登录里没有可用的会话令牌，请重新登录。".into())
}

fn find_session_key(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(key) = string_token(map.get("key")) {
                return Some(key);
            }
            if let Some(key) = string_token(map.get("access_token")) {
                return Some(key);
            }
            for (name, child) in map {
                if matches!(name.as_str(), "refresh_token" | "id_token") {
                    continue;
                }
                if let Some(found) = find_session_key(child) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_session_key),
        _ => None,
    }
}

fn string_token(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| item.len() >= 20)
        .map(ToOwned::to_owned)
}

fn billing_url() -> String {
    let base = std::env::var("GROK_CLI_CHAT_PROXY_BASE_URL")
        .ok()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| DEFAULT_PROXY_BASE.to_string());
    format!("{}/billing?format=credits", base.trim_end_matches('/'))
}

fn fetch_billing(token: &str, project: Option<&Project>, settings: &AppSettings) -> Result<String, String> {
    let mut last = "无法连接 Grok 用量接口。".to_string();
    for proxy in proxy_candidates(project, settings) {
        match request_billing(token, proxy.as_deref()) {
            Ok(body) => return Ok(body),
            Err(err) => last = err,
        }
    }
    Err(last)
}

fn proxy_candidates(project: Option<&Project>, settings: &AppSettings) -> Vec<Option<String>> {
    let mut out = Vec::new();
    if let Some(project) = project {
        if project.proxy_enabled {
            push_proxy(&mut out, &project.proxy_url);
        }
    }
    if out.is_empty() {
        push_proxy(&mut out, &settings.default_proxy_url);
    }
    if !out.iter().any(|item| item.is_none()) {
        out.push(None);
    }
    out
}

fn push_proxy(out: &mut Vec<Option<String>>, url: &str) {
    if let Ok(url) = validate_proxy_url(url) {
        if !out.iter().any(|item| item.as_deref() == Some(url.as_str())) {
            out.push(Some(url));
        }
    }
}

fn request_billing(token: &str, proxy: Option<&str>) -> Result<String, String> {
    let mut builder = ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT);
    if let Some(url) = proxy {
        let proxy = ureq::Proxy::new(url).map_err(|_| "代理地址无法用于读取用量。".to_string())?;
        builder = builder.proxy(proxy);
    }
    let response = builder.build().get(&billing_url())
        .set("Authorization", &format!("Bearer {token}"))
        .set("X-XAI-Token-Auth", "xai-grok-cli")
        .call();
    match response {
        Ok(resp) => read_ok_body(resp),
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp.into_string().unwrap_or_default())),
        Err(_) => Err(if proxy.is_some() {
            "走代理读取 Grok 用量失败。".into()
        } else {
            "无法连接 Grok 用量接口。".into()
        }),
    }
}

fn read_ok_body(resp: ureq::Response) -> Result<String, String> {
    let code = resp.status();
    let body = resp.into_string().map_err(|_| "Grok 用量返回无法读取。".to_string())?;
    if (200..300).contains(&code) {
        Ok(body)
    } else {
        Err(status_message(code, body))
    }
}

fn status_message(code: u16, body: String) -> String {
    match code {
        401 | 403 => "Grok 登录已过期，请重新登录。".into(),
        429 => "Grok 用量接口限流，请稍后再看。".into(),
        _ => {
            if body.trim().is_empty() {
                format!("读取 Grok 用量失败（{code}）。")
            } else {
                let snippet: String = body.chars().take(120).collect();
                format!("读取 Grok 用量失败（{code}）：{snippet}")
            }
        }
    }
}

fn parse_billing(body: &str) -> Result<GrokUsage, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "Grok 用量返回无法解析。".to_string())?;
    let config = value.get("config").unwrap_or(&value);
    let used = number_field(config, "creditUsagePercent").ok_or_else(|| "当前账号没有用量数据。".to_string())?;
    if !used.is_finite() {
        return Err("当前账号没有用量数据。".into());
    }
    let used = used.clamp(0.0, 100.0);
    let period = config.get("currentPeriod");
    let resets_at = period
        .and_then(|item| string_field(item, "end"))
        .or_else(|| string_field(config, "billingPeriodEnd"));
    let period_label = period
        .and_then(|item| string_field(item, "type"))
        .map(|item| period_label(&item));
    Ok(GrokUsage {
        ok: true,
        used_percent: Some(used),
        remaining_percent: Some((100.0 - used).clamp(0.0, 100.0)),
        resets_at,
        period_label,
        prepaid_balance: money_field(config, "prepaidBalance"),
        on_demand_used: money_field(config, "onDemandUsed"),
        on_demand_cap: money_field(config, "onDemandCap"),
        grok_build_used_percent: product_used(config, "GrokBuild"),
        used_credits: money_field(config, "used"),
        credit_limit: money_field(config, "monthlyLimit"),
        fetched_at: Utc::now().to_rfc3339(),
        message: None,
    })
}

fn period_label(raw: &str) -> String {
    let upper = raw.to_ascii_uppercase();
    if upper.contains("WEEK") {
        "本周".into()
    } else if upper.contains("MONTH") {
        "本月".into()
    } else if upper.contains("DAY") {
        "今日".into()
    } else {
        raw.to_string()
    }
}

fn product_used(config: &Value, name: &str) -> Option<f64> {
    config.get("productUsage")?.as_array()?.iter().find_map(|item| {
        let product = item.get("product").and_then(Value::as_str)?;
        if product.eq_ignore_ascii_case(name) {
            number_field(item, "usagePercent")
        } else {
            None
        }
    })
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    as_number(value.get(key)?)
}

fn money_field(value: &Value, key: &str) -> Option<f64> {
    let field = value.get(key)?;
    if let Some(amount) = field.get("val") {
        return as_number(amount);
    }
    as_number(field)
}

fn as_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|item| item as f64))
        .or_else(|| value.as_u64().map(|item| item as f64))
        .or_else(|| value.as_str().and_then(|item| item.trim().parse().ok()))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::trim).filter(|item| !item.is_empty()).map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_cli_session_key() {
        let value = json!({
            "https://auth.x.ai::abc": {
                "key": "session-token-value-123456",
                "refresh_token": "refresh-token-should-be-ignored-xx",
                "email": "dev@example.com"
            }
        });
        assert_eq!(find_session_key(&value).as_deref(), Some("session-token-value-123456"));
    }

    #[test]
    fn parse_weekly_credits_panel() {
        let body = r#"{
            "config":{
                "currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","end":"2026-08-15T01:53:09.930537+00:00"},
                "creditUsagePercent":42.4,
                "onDemandCap":{"val":0},
                "onDemandUsed":{"val":0},
                "productUsage":[{"product":"GrokBuild","usagePercent":80.0},{"product":"GrokChat"}],
                "prepaidBalance":{"val":"12.5"},
                "billingPeriodEnd":"2026-08-15T01:53:09.930537+00:00"
            }
        }"#;
        let usage = parse_billing(body).unwrap();
        assert!(usage.ok);
        assert_eq!(usage.used_percent, Some(42.4));
        assert_eq!(usage.remaining_percent, Some(57.6));
        assert_eq!(usage.period_label.as_deref(), Some("本周"));
        assert_eq!(usage.prepaid_balance, Some(12.5));
        assert_eq!(usage.grok_build_used_percent, Some(80.0));
        assert_eq!(usage.used_credits, None);
        assert_eq!(usage.credit_limit, None);
        assert_eq!(usage.resets_at.as_deref(), Some("2026-08-15T01:53:09.930537+00:00"));
    }

    #[test]
    fn parse_optional_credit_counts() {
        let body = r#"{
            "config":{
                "creditUsagePercent":7.1,
                "used":{"val":4277},
                "monthlyLimit":{"val":60000}
            }
        }"#;
        let usage = parse_billing(body).unwrap();
        assert_eq!(usage.used_credits, Some(4277.0));
        assert_eq!(usage.credit_limit, Some(60000.0));
    }

    #[test]
    fn missing_percent_is_unavailable() {
        let err = parse_billing(r#"{"config":{}}"#).unwrap_err();
        assert!(err.contains("没有用量数据"));
    }
}
