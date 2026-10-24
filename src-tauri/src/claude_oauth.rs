use crate::proxy::validate_proxy_url;
use crate::state::AppSettings;
use chrono::Utc;
use serde_json::Value;
use std::fmt;
use std::time::Duration;

const PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
#[derive(Debug, Clone, PartialEq)]
pub enum AuthError {
    Empty,
    Rejected,
    Http(&'static str, u16),
    Network,
    InvalidResponse,
    IdentityMismatch,
}

impl AuthError {
    #[cfg(test)]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty_credentials",
            Self::Rejected => "login_required",
            Self::Http(_, _) => "http_error",
            Self::Network => "network_error",
            Self::InvalidResponse => "invalid_response",
            Self::IdentityMismatch => "identity_mismatch",
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("这个账号的登录凭据为空，请重新登录后保存。"),
            Self::Rejected => f.write_str("此账号未通过身份检查，请重新登录后重试。"),
            Self::Http(stage, status) => {
                write!(f, "Claude {stage}接口返回 HTTP {status}，请稍后重试。")
            }
            Self::Network => f.write_str("无法连接 Claude 认证接口，请检查代理或网络后重试。"),
            Self::InvalidResponse => f.write_str("Claude 认证接口返回的数据不完整，请稍后重试。"),
            Self::IdentityMismatch => f.write_str(
                "账号信息与登录凭据不属于同一账号，已停止操作。请重新登录目标账号后保存。",
            ),
        }
    }
}

pub trait OAuthClient {
    fn profile(&self, oauth: &Value) -> Result<Value, AuthError>;
}

pub struct HttpOAuthClient {
    agent: ureq::Agent,
}

impl HttpOAuthClient {
    pub fn new(settings: &AppSettings) -> Result<Self, String> {
        let mut builder = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .redirects(0);
        if !settings.default_proxy_url.trim().is_empty() {
            let url = validate_proxy_url(&settings.default_proxy_url)?;
            builder =
                builder.proxy(ureq::Proxy::new(url).map_err(|_| "Claude 认证代理地址无效。")?);
        }
        Ok(Self {
            agent: builder.build(),
        })
    }
}

impl OAuthClient for HttpOAuthClient {
    fn profile(&self, oauth: &Value) -> Result<Value, AuthError> {
        let token = token(oauth, "accessToken").ok_or(AuthError::Empty)?;
        response_json(
            self.agent
                .get(PROFILE_URL)
                .set("Authorization", &format!("Bearer {token}"))
                .set("anthropic-beta", "oauth-2025-04-20")
                .call(),
            "身份核验",
        )
    }
}

fn response_json(
    result: Result<ureq::Response, ureq::Error>,
    stage: &'static str,
) -> Result<Value, AuthError> {
    match result {
        Ok(response) if response.status() == 200 => {
            response.into_json().map_err(|_| AuthError::InvalidResponse)
        }
        Ok(response) => Err(AuthError::Http(stage, response.status())),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_json::<Value>().unwrap_or(Value::Null);
            if status == 401 || body.get("error").and_then(Value::as_str) == Some("invalid_grant") {
                Err(AuthError::Rejected)
            } else {
                Err(AuthError::Http(stage, status))
            }
        }
        // Raw errors/bodies can include URLs, headers or token material.
        Err(ureq::Error::Transport(_)) => Err(AuthError::Network),
    }
}

pub fn token<'a>(oauth: &'a Value, field: &str) -> Option<&'a str> {
    oauth
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| s.bytes().all(|b| (0x21..=0x7e).contains(&b)))
}

pub fn has_credentials(oauth: &Value) -> bool {
    token(oauth, "accessToken").is_some() || token(oauth, "refreshToken").is_some()
}

pub fn needs_refresh(oauth: &Value) -> bool {
    token(oauth, "accessToken").is_none()
        || oauth
            .get("expiresAt")
            .and_then(Value::as_i64)
            .is_some_and(|expiry| expiry <= Utc::now().timestamp_millis() + 60_000)
}

pub fn verify_identity(account: &Value, profile: &Value) -> Result<(), AuthError> {
    let actual = profile.get("account").ok_or(AuthError::InvalidResponse)?;
    let actual_uuid = token(actual, "uuid").ok_or(AuthError::InvalidResponse)?;
    let matches = match token(account, "accountUuid") {
        Some(expected) => expected == actual_uuid,
        None => token(account, "emailAddress")
            .zip(token(actual, "email"))
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)),
    };
    if matches {
        Ok(())
    } else {
        Err(AuthError::IdentityMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_empty_tokens_and_requests_refresh_for_expired_access() {
        assert!(!has_credentials(&json!({"accessToken": "bad\ntoken"})));
        assert!(!has_credentials(
            &json!({"accessToken": " ", "refreshToken": ""})
        ));
        assert!(has_credentials(&json!({"refreshToken": "refresh"})));
        assert!(needs_refresh(
            &json!({"accessToken": "access", "expiresAt": 0})
        ));
    }

    #[test]
    fn uuid_mismatch_is_rejected_even_when_email_matches() {
        let account = json!({"accountUuid": "a", "emailAddress": "ada@example.com"});
        assert_eq!(
            verify_identity(
                &account,
                &json!({"account": {"uuid": "b", "email": "ada@example.com"}})
            ),
            Err(AuthError::IdentityMismatch)
        );
        assert!(verify_identity(&account, &json!({"account": {"uuid": "a"}})).is_ok());
        assert_eq!(
            verify_identity(&account, &json!({})),
            Err(AuthError::InvalidResponse)
        );
    }

    #[test]
    fn http_errors_never_include_server_supplied_secrets() {
        let response = ureq::Response::new(
            400,
            "Bad Request",
            r#"{"error":"invalid_grant","error_description":"secret-refresh-token"}"#,
        )
        .unwrap();
        let err = response_json(Err(ureq::Error::Status(400, response)), "续期").unwrap_err();
        assert_eq!(err, AuthError::Rejected);
        assert!(!err.to_string().contains("secret-refresh-token"));
        let response =
            ureq::Response::new(429, "Rate Limited", r#"{"message":"secret-access-token"}"#)
                .unwrap();
        let err = response_json(Err(ureq::Error::Status(429, response)), "续期").unwrap_err();
        assert_eq!(err, AuthError::Http("续期", 429));
        assert!(!err.to_string().contains("secret-access-token"));
    }

    #[test]
    #[ignore = "Read-only live check; requires explicit credential and app-state file paths"]
    fn live_profile_probe() {
        let credentials_path = std::env::var("AGENT_DOCK_AUTH_PROBE_CREDENTIALS")
            .expect("set AGENT_DOCK_AUTH_PROBE_CREDENTIALS to a local credential file path");
        let state_path = std::env::var("AGENT_DOCK_AUTH_PROBE_STATE")
            .expect("set AGENT_DOCK_AUTH_PROBE_STATE to the app state file path");
        let original = std::fs::read(&credentials_path).expect("read credentials");
        let credentials: Value = serde_json::from_slice(&original).expect("parse credentials");
        let state: Value =
            serde_json::from_slice(&std::fs::read(state_path).expect("read app state"))
                .expect("parse app state");
        let mut settings = AppSettings::default();
        settings.default_proxy_url = state["settings"]["defaultProxyUrl"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let client = HttpOAuthClient::new(&settings).expect("configure OAuth client");
        let profile = client
            .profile(&credentials["claudeAiOauth"])
            .unwrap_or_else(|err| panic!("profile probe failed: {}", err.code()));
        assert!(token(&profile["account"], "uuid").is_some());
        let after = std::fs::read(&credentials_path).expect("re-read credentials");
        assert!(original == after, "the probe must not write credentials");
    }
}
