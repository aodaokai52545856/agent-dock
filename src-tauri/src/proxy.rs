const DEFAULT_PROXY: &str = "http://127.0.0.1:7890";

pub fn default_proxy_url() -> String {
    DEFAULT_PROXY.to_string()
}

pub fn validate_proxy_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("代理地址不能为空，请填写 http:// 或 https:// 开头的地址".into());
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("代理地址格式不正确，请使用 http://127.0.0.1:7890 这种形式".into());
    }
    if trimmed.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '\n' | '\r')) {
        return Err("代理地址不能包含空格或引号，请只填写 URL".into());
    }
    if trimmed.len() > 256 {
        return Err("代理地址过长，请检查是否粘贴了多余内容".into());
    }
    Ok(trimmed.to_string())
}

pub fn proxy_env(enabled: bool, url: &str) -> Result<Vec<(String, String)>, String> {
    if !enabled {
        return Ok(Vec::new());
    }
    let url = validate_proxy_url(url)?;
    Ok(vec![
        ("HTTP_PROXY".into(), url.clone()),
        ("HTTPS_PROXY".into(), url.clone()),
        ("http_proxy".into(), url.clone()),
        ("https_proxy".into(), url),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_default_clash_url() {
        let env = proxy_env(true, "http://127.0.0.1:7890").unwrap();
        assert_eq!(env.len(), 4);
        assert!(env.iter().all(|(_, v)| v == "http://127.0.0.1:7890"));
    }

    #[test]
    fn disabled_adds_nothing() {
        assert!(proxy_env(false, "http://127.0.0.1:7890").unwrap().is_empty());
    }

    #[test]
    fn rejects_missing_scheme() {
        let err = validate_proxy_url("127.0.0.1:7890").unwrap_err();
        assert!(err.contains("http://"));
    }

    #[test]
    fn rejects_quotes() {
        assert!(validate_proxy_url("http://127.0.0.1:7890\"").is_err());
    }
}
