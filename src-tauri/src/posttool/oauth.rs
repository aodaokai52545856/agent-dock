use super::*;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Callback {
    pub id: String,
    pub state: String,
    pub redirect_uri: String,
    pub code: Option<String>,
    pub error: Option<String>,
    pub done: bool,
}
struct Flow {
    callback: Arc<Mutex<Callback>>,
    cancel: CancellationToken,
}
static FLOWS: OnceLock<Mutex<HashMap<String, Flow>>> = OnceLock::new();
fn flows() -> &'static Mutex<HashMap<String, Flow>> {
    FLOWS.get_or_init(|| Mutex::new(HashMap::new()))
}
#[tauri::command]
pub async fn posttool_oauth_start(redirect_uri: String) -> Result<Callback, String> {
    let mut url = reqwest::Url::parse(if redirect_uri.is_empty() {
        "http://127.0.0.1:0/oauth/callback"
    } else {
        &redirect_uri
    })
    .map_err(err)?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "OAuth 回调需要 http://127.0.0.1:端口/路径 或 http://localhost:端口/路径".into(),
        );
    }
    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, url.port().unwrap_or(0)))
            .await
            .map_err(|e| format!("回调端口无法监听：{e}"))?;
    url.set_port(Some(listener.local_addr().map_err(err)?.port()))
        .map_err(|_| "回调端口无效")?;
    let callback = Callback {
        id: uuid::Uuid::new_v4().to_string(),
        state: uuid::Uuid::new_v4().to_string(),
        redirect_uri: url.to_string(),
        code: None,
        error: None,
        done: false,
    };
    let id = callback.id.clone();
    let shared = Arc::new(Mutex::new(callback.clone()));
    let token = CancellationToken::new();
    {
        let mut all = flows().lock().unwrap();
        all.retain(|_, f| !f.callback.lock().unwrap().done);
        if all.len() >= 2 {
            return Err("同时最多 2 个 OAuth 授权流程".into());
        }
        all.insert(
            id,
            Flow {
                callback: shared.clone(),
                cancel: token.clone(),
            },
        );
    }
    tauri::async_runtime::spawn(async move {
        let expected_path = url.path().to_string();
        let outcome = tokio::select! {biased;_=token.cancelled()=>Err("授权已取消".into()),result=tokio::time::timeout(Duration::from_secs(180),async{
            for _ in 0..16{let(mut socket,_)=listener.accept().await.map_err(err)?;let mut bytes=vec![0;8192];let n=tokio::time::timeout(Duration::from_secs(2),socket.read(&mut bytes)).await.map_err(|_|"回调读取超时")?.map_err(err)?;let text=String::from_utf8_lossy(&bytes[..n]);let target=text.lines().next().and_then(|line|line.strip_prefix("GET ")).and_then(|line|line.split_whitespace().next()).unwrap_or("");let received=reqwest::Url::parse(&format!("http://127.0.0.1{target}"));
                let expected=shared.lock().unwrap().state.clone();let values=received.as_ref().ok().map(|u|u.query_pairs().into_owned().collect::<HashMap<_,_>>()).unwrap_or_default();
                let valid=received.as_ref().is_ok_and(|u|u.path()==expected_path)&&values.get("state")==Some(&expected);
                let body=if valid{"Authorization received. You may close this window and return to Agent Dock."}else{"Invalid OAuth callback state or path."};let status=if valid{"200 OK"}else{"400 Bad Request"};let response=format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());let _=socket.write_all(response.as_bytes()).await;
                if valid{if let Some(error)=values.get("error"){return Err(format!("授权失败：{error}"))}return values.get("code").cloned().filter(|v|!v.is_empty()).ok_or("回调缺少 code".into())}
            }Err("无效回调次数过多".into())
        })=>result.unwrap_or_else(|_|Err("授权超过 180 秒，回调端口已关闭".into()))};
        let mut result = shared.lock().unwrap();
        result.done = true;
        match outcome {
            Ok(code) => result.code = Some(code),
            Err(error) => result.error = Some(error),
        }
    });
    Ok(callback)
}
#[tauri::command]
pub fn posttool_oauth_poll(id: String) -> Result<Callback, String> {
    flows()
        .lock()
        .unwrap()
        .get(&id)
        .map(|f| f.callback.lock().unwrap().clone())
        .ok_or("授权流程已释放".into())
}
#[tauri::command]
pub fn posttool_oauth_close(id: String) {
    if let Some(flow) = flows().lock().unwrap().remove(&id) {
        flow.cancel.cancel()
    }
}
pub fn shutdown() {
    if let Some(all) = FLOWS.get() {
        for flow in all.lock().unwrap().values() {
            flow.cancel.cancel()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn callback_validates_state_then_closes_listener() {
        let flow = posttool_oauth_start(String::new()).await.unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let invalid = client
            .get(format!("{}?state=wrong&code=bad", flow.redirect_uri))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), 400);
        assert!(!posttool_oauth_poll(flow.id.clone()).unwrap().done);
        let valid = client
            .get(format!(
                "{}?state={}&code=verified-code",
                flow.redirect_uri, flow.state
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(valid.status(), 200);
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if posttool_oauth_poll(flow.id.clone()).unwrap().done {
                    break;
                }
                tokio::task::yield_now().await
            }
        })
        .await
        .unwrap();
        let result = posttool_oauth_poll(flow.id.clone()).unwrap();
        assert_eq!(result.code.as_deref(), Some("verified-code"));
        posttool_oauth_close(flow.id);
        assert!(posttool_oauth_start("http://0.0.0.0/callback".into())
            .await
            .is_err());
    }
}
