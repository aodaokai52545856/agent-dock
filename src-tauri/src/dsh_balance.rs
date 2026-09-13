//! DeepSeek prepaid balance (CNY / USD).
//!
//! Official endpoint: `GET https://api.deepseek.com/user/balance`

use crate::dsh_credentials;
use crate::dsh_keys;
use crate::proxy::validate_proxy_url;
use crate::state::{self, AppSettings, Project};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tauri::AppHandle;

const DEFAULT_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DshBalance {
    pub ok: bool,
    pub available: Option<bool>,
    pub currency: Option<String>,
    pub total_balance: Option<f64>,
    pub granted_balance: Option<f64>,
    pub topped_up_balance: Option<f64>,
    pub fetched_at: String,
    pub message: Option<String>,
}

impl DshBalance {
    fn fail(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            available: None,
            currency: None,
            total_balance: None,
            granted_balance: None,
            topped_up_balance: None,
            fetched_at: Utc::now().to_rfc3339(),
            message: Some(message.into()),
        }
    }
}

pub fn fetch(app: &AppHandle, project_id: Option<&str>) -> DshBalance {
    let state = match state::load_state(app) {
        Ok(state) => state,
        Err(message) => return DshBalance::fail(message),
    };
    let project = project_id.and_then(|id| state::find_project(&state, id).ok());
    let project_dir = project.map(|item| Path::new(item.path.as_str()));
    let token = match api_key(app, project_dir) {
        Ok(token) => token,
        Err(message) => return DshBalance::fail(message),
    };
    match fetch_balance(&token, project, &state.settings) {
        Ok(body) => match parse_balance(&body) {
            Ok(balance) => balance,
            Err(message) => DshBalance::fail(message),
        },
        Err(message) => DshBalance::fail(message),
    }
}

fn api_key(app: &AppHandle, project_dir: Option<&Path>) -> Result<String, String> {
    if let Some(secret) = dsh_keys::active_secret(app) {
        let secret = secret.trim();
        if !secret.is_empty() {
            return Ok(secret.to_string());
        }
    }
    dsh_credentials::live_secret(project_dir)
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "没有可用的 DeepSeek API Key。请先在 DeepSeek Key 里添加。".into())
}

fn fetch_balance(token: &str, project: Option<&Project>, settings: &AppSettings) -> Result<String, String> {
    let mut last = "无法连接 DeepSeek 余额接口。".to_string();
    for proxy in proxy_candidates(project, settings) {
        match request_balance(token, proxy.as_deref()) {
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

fn request_balance(token: &str, proxy: Option<&str>) -> Result<String, String> {
    let mut builder = ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT);
    if let Some(url) = proxy {
        let proxy = ureq::Proxy::new(url).map_err(|_| "代理地址无法用于读取余额。".to_string())?;
        builder = builder.proxy(proxy);
    }
    let response = builder
        .build()
        .get(DEFAULT_BALANCE_URL)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json")
        .call();
    match response {
        Ok(resp) => read_ok_body(resp),
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp.into_string().unwrap_or_default())),
        Err(_) => Err(if proxy.is_some() {
            "走代理读取 DeepSeek 余额失败。".into()
        } else {
            "无法连接 DeepSeek 余额接口。".into()
        }),
    }
}

fn read_ok_body(resp: ureq::Response) -> Result<String, String> {
    let code = resp.status();
    let body = resp.into_string().map_err(|_| "DeepSeek 余额返回无法读取。".to_string())?;
    if (200..300).contains(&code) {
        Ok(body)
    } else {
        Err(status_message(code, body))
    }
}

fn status_message(code: u16, body: String) -> String {
    match code {
        401 | 403 => "DeepSeek API Key 无效或已过期。".into(),
        429 => "DeepSeek 余额接口限流，请稍后再看。".into(),
        _ => {
            if body.trim().is_empty() {
                format!("读取 DeepSeek 余额失败（{code}）。")
            } else {
                let snippet: String = body.chars().take(120).collect();
                format!("读取 DeepSeek 余额失败（{code}）：{snippet}")
            }
        }
    }
}

fn parse_balance(body: &str) -> Result<DshBalance, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "DeepSeek 余额返回无法解析。".to_string())?;
    let infos = value
        .get("balance_infos")
        .and_then(Value::as_array)
        .ok_or_else(|| "当前账号没有余额数据。".to_string())?;
    let info = pick_info(infos).ok_or_else(|| "当前账号没有余额数据。".to_string())?;
    let total = money_field(info, "total_balance").ok_or_else(|| "当前账号没有余额数据。".to_string())?;
    if !total.is_finite() {
        return Err("当前账号没有余额数据。".into());
    }
    Ok(DshBalance {
        ok: true,
        available: value.get("is_available").and_then(Value::as_bool),
        currency: string_field(info, "currency"),
        total_balance: Some(total),
        granted_balance: money_field(info, "granted_balance"),
        topped_up_balance: money_field(info, "topped_up_balance"),
        fetched_at: Utc::now().to_rfc3339(),
        message: None,
    })
}

fn pick_info(infos: &[Value]) -> Option<&Value> {
    infos
        .iter()
        .find(|item| currency_is(item, "CNY"))
        .or_else(|| infos.iter().find(|item| currency_is(item, "USD")))
        .or_else(|| infos.first())
}

fn currency_is(value: &Value, want: &str) -> bool {
    value
        .get("currency")
        .and_then(Value::as_str)
        .map(|item| item.eq_ignore_ascii_case(want))
        .unwrap_or(false)
}

fn money_field(value: &Value, key: &str) -> Option<f64> {
    as_number(value.get(key)?)
}

fn as_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|item| item as f64))
        .or_else(|| value.as_u64().map(|item| item as f64))
        .or_else(|| value.as_str().and_then(|item| item.trim().parse().ok()))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_cny_over_usd() {
        let body = r#"{
            "is_available": true,
            "balance_infos": [
                {"currency":"USD","total_balance":"12.00","granted_balance":"0.00","topped_up_balance":"12.00"},
                {"currency":"CNY","total_balance":"110.00","granted_balance":"10.00","topped_up_balance":"100.00"}
            ]
        }"#;
        let balance = parse_balance(body).unwrap();
        assert!(balance.ok);
        assert_eq!(balance.available, Some(true));
        assert_eq!(balance.currency.as_deref(), Some("CNY"));
        assert_eq!(balance.total_balance, Some(110.0));
        assert_eq!(balance.granted_balance, Some(10.0));
        assert_eq!(balance.topped_up_balance, Some(100.0));
    }

    #[test]
    fn falls_back_to_usd_when_no_cny() {
        let body = r#"{
            "is_available": true,
            "balance_infos": [
                {"currency":"USD","total_balance":"8.5","granted_balance":"0","topped_up_balance":"8.5"}
            ]
        }"#;
        let balance = parse_balance(body).unwrap();
        assert_eq!(balance.currency.as_deref(), Some("USD"));
        assert_eq!(balance.total_balance, Some(8.5));
    }

    #[test]
    fn empty_infos_is_unavailable() {
        let err = parse_balance(r#"{"is_available":false,"balance_infos":[]}"#).unwrap_err();
        assert!(err.contains("没有余额数据"));
    }

    #[test]
    fn malformed_json_is_unavailable() {
        let err = parse_balance("not-json").unwrap_err();
        assert!(err.contains("无法解析"));
    }

    #[test]
    fn keeps_zero_balance_when_unavailable() {
        let body = r#"{
            "is_available": false,
            "balance_infos": [
                {"currency":"CNY","total_balance":"0.00","granted_balance":"0.00","topped_up_balance":"0.00"}
            ]
        }"#;
        let balance = parse_balance(body).unwrap();
        assert!(balance.ok);
        assert_eq!(balance.available, Some(false));
        assert_eq!(balance.total_balance, Some(0.0));
    }
}
