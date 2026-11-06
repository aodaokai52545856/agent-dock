//! Local API client. No workers, clients, database or cache are created until first use.
pub mod grpc;
pub mod oauth;
pub mod storage;
pub mod streams;

use reqwest::{
    header::{HeaderName, HeaderValue},
    Client, Method,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::{io::ReaderStream, sync::CancellationToken};

const CACHE_LIMIT: u64 = 512 * 1024 * 1024;
const RESPONSE_LIMIT: u64 = 128 * 1024 * 1024;
const MAX_RESPONSES: usize = 24;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub key: String,
    pub value: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub file: bool,
}
fn yes() -> bool {
    true
}
fn default_timeout() -> u64 {
    30_000
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSpec {
    pub id: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<Pair>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub body_mode: String,
    #[serde(default)]
    pub fields: Vec<Pair>,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub proxy: String,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default = "yes")]
    pub follow_redirects: bool,
    #[serde(default = "yes")]
    pub cookies: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub ca_path: String,
    #[serde(default)]
    pub identity_path: String,
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMeta {
    pub id: String,
    pub state: String,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<Pair>,
    pub url: String,
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub first_byte_ms: u64,
    pub error: Option<String>,
    pub content_type: String,
}
struct ResponseEntry {
    meta: Arc<Mutex<ResponseMeta>>,
    path: PathBuf,
    started: Instant,
}
struct Inner {
    active: HashMap<String, CancellationToken>,
    responses: HashMap<String, ResponseEntry>,
    order: VecDeque<String>,
    cancelled: VecDeque<(String, Instant)>,
    clients: VecDeque<(String, Client)>,
}
struct CacheDirectory {
    path: PathBuf,
    _lock: std::fs::File,
}
pub struct Hub {
    inner: Mutex<Inner>,
    bytes: AtomicU64,
    cache: Mutex<Option<CacheDirectory>>,
    cookies: Arc<BoundedCookies>,
}
#[derive(Default)]
struct BoundedCookies(Mutex<cookie_store::CookieStore>);
impl reqwest::cookie::CookieStore for BoundedCookies {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &reqwest::Url) {
        let mut store = self.0.lock().unwrap();
        for header in headers.take(512) {
            let Ok(value) = header.to_str() else { continue };
            if value.len() > 4096 {
                continue;
            }
            if store.iter_any().count() >= 512 {
                store.clear()
            }
            let _ = store.parse(value, url);
        }
    }
    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        let store = self.0.lock().unwrap();
        let mut value = String::new();
        for (name, item) in store.get_request_values(url) {
            if value.len() + name.len() + item.len() + 3 > 32 * 1024 {
                break;
            }
            if !value.is_empty() {
                value.push_str("; ")
            }
            value.push_str(name);
            value.push('=');
            value.push_str(item)
        }
        if value.is_empty() {
            None
        } else {
            HeaderValue::from_str(&value).ok()
        }
    }
}
static HUB: OnceLock<Arc<Hub>> = OnceLock::new();
fn hub() -> &'static Arc<Hub> {
    HUB.get_or_init(|| {
        Arc::new(Hub {
            inner: Mutex::new(Inner {
                active: HashMap::new(),
                responses: HashMap::new(),
                order: VecDeque::new(),
                cancelled: VecDeque::new(),
                clients: VecDeque::new(),
            }),
            bytes: AtomicU64::new(0),
            cache: Mutex::new(None),
            cookies: Arc::new(BoundedCookies::default()),
        })
    })
}
pub fn shutdown() {
    streams::shutdown();
    oauth::shutdown();
    if let Some(hub) = HUB.get() {
        for token in hub.inner.lock().unwrap().active.values() {
            token.cancel();
        }
    }
}
fn root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("posttool"))
}
fn validate_id(id: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "请求 ID 无效".into())
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn tls_provider() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
fn cache_directory(base: &Path, h: &Hub) -> Result<PathBuf, String> {
    let mut cache = h.cache.lock().unwrap();
    if let Some(existing) = cache.as_ref() {
        return Ok(existing.path.clone());
    }
    std::fs::create_dir_all(base).map_err(err)?;
    // An exclusive OS lock distinguishes abandoned cache directories from live instances.
    for entry in std::fs::read_dir(base).map_err(err)?.flatten() {
        let path = entry.path();
        if !entry.file_type().map_err(err)?.is_dir()
            || uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err()
        {
            continue;
        }
        let Ok(lock) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.join(".lock"))
        else {
            continue;
        };
        if lock.try_lock().is_err() {
            continue;
        }
        for file in std::fs::read_dir(&path).map_err(err)?.flatten() {
            if file.file_type().map_err(err)?.is_file()
                && file.path().extension().is_some_and(|e| e == "body")
            {
                let _ = std::fs::remove_file(file.path());
            }
        }
        drop(lock);
        let _ = std::fs::remove_file(path.join(".lock"));
        let _ = std::fs::remove_dir(path);
    }
    let path = base.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir(&path).map_err(err)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path.join(".lock"))
        .map_err(err)?;
    lock.try_lock().map_err(err)?;
    *cache = Some(CacheDirectory {
        path: path.clone(),
        _lock: lock,
    });
    Ok(path)
}

struct ActiveGuard {
    id: String,
    hub: Arc<Hub>,
}
struct Reservation<'a> {
    bytes: &'a AtomicU64,
    count: u64,
    committed: bool,
}
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.bytes.fetch_sub(self.count, Ordering::SeqCst);
        }
    }
}
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.hub.inner.lock().unwrap().active.remove(&self.id);
    }
}
fn forget(inner: &mut Inner, id: &str, hub: &Hub) {
    if let Some(entry) = inner.responses.remove(id) {
        let bytes = entry.meta.lock().unwrap().bytes;
        if std::fs::remove_file(&entry.path).is_ok() {
            hub.bytes.fetch_sub(bytes, Ordering::SeqCst);
        }
    }
    inner.order.retain(|x| x != id);
}

#[tauri::command]
pub async fn posttool_request(app: AppHandle, spec: RequestSpec) -> Result<ResponseMeta, String> {
    validate_id(&spec.id)?;
    if spec.body.len() > 2 * 1024 * 1024 || spec.headers.len() > 512 || spec.fields.len() > 512 {
        return Err("请求超过编辑上限，请使用文件正文".into());
    }
    let h = hub().clone();
    let token = CancellationToken::new();
    let start = Instant::now();
    {
        let mut inner = h.inner.lock().unwrap();
        if inner.active.contains_key(&spec.id) || inner.responses.contains_key(&spec.id) {
            return Err("请求 ID 已使用".into());
        }
        if inner.active.len() >= 8 {
            return Err("同时最多运行 8 个请求，请等待或取消其他请求".into());
        }
        if inner
            .cancelled
            .iter()
            .any(|(id, at)| id == &spec.id && at.elapsed() < Duration::from_secs(30))
        {
            return Err("请求已取消".into());
        }
        while inner.responses.len() >= MAX_RESPONSES {
            let old = inner
                .order
                .iter()
                .find(|id| !inner.active.contains_key(*id))
                .cloned()
                .ok_or("响应缓存已满")?;
            forget(&mut inner, &old, &h);
        }
        inner.active.insert(spec.id.clone(), token.clone());
    }
    let _guard = ActiveGuard {
        id: spec.id.clone(),
        hub: h.clone(),
    };
    let base = root(&app)?.join("responses");
    let cache_hub = h.clone();
    let dir = tauri::async_runtime::spawn_blocking(move || cache_directory(&base, &cache_hub))
        .await
        .map_err(err)??;
    let path = dir.join(format!("{}.body", spec.id));
    let meta = Arc::new(Mutex::new(ResponseMeta {
        id: spec.id.clone(),
        state: "connecting".into(),
        ..Default::default()
    }));
    {
        let mut inner = h.inner.lock().unwrap();
        inner.responses.insert(
            spec.id.clone(),
            ResponseEntry {
                meta: meta.clone(),
                path: path.clone(),
                started: start,
            },
        );
        inner.order.push_back(spec.id.clone());
    }
    let timeout = Duration::from_millis(spec.timeout_ms.clamp(100, 3_600_000));
    let outcome = tokio::select! {
        biased;
        _ = token.cancelled() => Err("请求已取消".to_string()),
        result = tokio::time::timeout(timeout, execute(&spec, &path, &meta, &h)) => result.unwrap_or_else(|_| Err("请求总时限已到，连接已终止".into())),
    };
    let mut done = meta.lock().unwrap();
    done.state = if outcome.is_ok() {
        "done"
    } else if token.is_cancelled() {
        "cancelled"
    } else {
        "error"
    }
    .into();
    done.elapsed_ms = start.elapsed().as_millis() as u64;
    done.error = outcome.err();
    Ok(done.clone())
}

async fn execute(
    spec: &RequestSpec,
    path: &Path,
    meta: &Arc<Mutex<ResponseMeta>>,
    h: &Arc<Hub>,
) -> Result<(), String> {
    tls_provider();
    let url = reqwest::Url::parse(&spec.url).map_err(|e| format!("URL 无效：{e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只支持 HTTP/HTTPS，请填写完整 URL".into());
    }
    let key = format!(
        "{}|{}|{}|{}|{}|{}",
        spec.proxy,
        spec.insecure,
        spec.follow_redirects,
        spec.cookies,
        spec.ca_path,
        spec.identity_path
    );
    let cached = {
        h.inner
            .lock()
            .unwrap()
            .clients
            .iter()
            .find(|(k, _)| k == &key)
            .map(|(_, c)| c.clone())
    };
    let client = if let Some(client) = cached {
        client
    } else {
        let mut builder = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .danger_accept_invalid_certs(spec.insecure)
            .redirect(if spec.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            });
        if spec.cookies {
            builder = builder.cookie_provider(h.cookies.clone());
        }
        if spec.proxy == "direct" {
            builder = builder.no_proxy();
        } else if !spec.proxy.trim().is_empty() {
            builder = builder.proxy(reqwest::Proxy::all(&spec.proxy).map_err(err)?);
        }
        if !spec.ca_path.is_empty() {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(&read_limited(&spec.ca_path, 1024 * 1024).await?)
                    .map_err(err)?,
            );
        }
        if !spec.identity_path.is_empty() {
            builder = builder.identity(
                reqwest::Identity::from_pem(&read_limited(&spec.identity_path, 1024 * 1024).await?)
                    .map_err(err)?,
            );
        }
        let client = builder.build().map_err(err)?;
        let mut inner = h.inner.lock().unwrap();
        inner.clients.push_back((key, client.clone()));
        while inner.clients.len() > 4 {
            inner.clients.pop_front();
        }
        client
    };
    let mut req = client.request(
        Method::from_bytes(spec.method.as_bytes()).map_err(err)?,
        url,
    );
    for pair in spec
        .headers
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
    {
        req = req.header(
            HeaderName::from_bytes(pair.key.as_bytes())
                .map_err(|e| format!("Header {}：{e}", pair.key))?,
            HeaderValue::from_str(&pair.value).map_err(|e| format!("Header {}：{e}", pair.key))?,
        );
    }
    match spec.body_mode.as_str() {
        "binary" => {
            let file = tokio::fs::File::open(&spec.file_path)
                .await
                .map_err(|e| format!("读取上传文件失败：{e}"))?;
            let len = file.metadata().await.map_err(err)?.len();
            req = req
                .header("Content-Length", len)
                .body(reqwest::Body::wrap_stream(ReaderStream::new(file)));
        }
        "multipart" => {
            let mut form = reqwest::multipart::Form::new();
            for pair in spec
                .fields
                .iter()
                .filter(|p| p.enabled && !p.key.is_empty())
            {
                if pair.file {
                    let file = tokio::fs::File::open(&pair.value)
                        .await
                        .map_err(|e| format!("读取上传文件失败：{e}"))?;
                    let len = file.metadata().await.map_err(err)?.len();
                    let part = reqwest::multipart::Part::stream_with_length(
                        reqwest::Body::wrap_stream(ReaderStream::new(file)),
                        len,
                    )
                    .file_name(
                        Path::new(&pair.value)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    );
                    form = form.part(pair.key.clone(), part);
                } else {
                    form = form.text(pair.key.clone(), pair.value.clone());
                }
            }
            req = req.multipart(form);
        }
        "urlencoded" => {
            let pairs: Vec<_> = spec
                .fields
                .iter()
                .filter(|p| p.enabled && !p.key.is_empty())
                .map(|p| (&p.key, &p.value))
                .collect();
            req = req.form(&pairs);
        }
        "none" | "" => {}
        _ => {
            req = req.body(spec.body.clone());
        }
    }
    let mut response = req.send().await.map_err(|e| format!("连接失败：{e}"))?;
    let first_byte_ms = h
        .inner
        .lock()
        .unwrap()
        .responses
        .get(&spec.id)
        .map(|e| e.started.elapsed().as_millis() as u64)
        .unwrap_or(0);
    {
        let mut m = meta.lock().unwrap();
        m.state = "receiving".into();
        m.status = response.status().as_u16();
        m.status_text = response.status().canonical_reason().unwrap_or("").into();
        m.url = response.url().to_string();
        m.first_byte_ms = first_byte_ms;
        m.headers = response
            .headers()
            .iter()
            .map(|(k, v)| Pair {
                key: k.to_string(),
                value: v.to_str().unwrap_or("[binary]").into(),
                enabled: true,
                file: false,
            })
            .collect();
        m.content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into();
    }
    let mut file = tokio::fs::File::create(path).await.map_err(err)?;
    while let Some(bytes) = response
        .chunk()
        .await
        .map_err(|e| format!("响应读取失败：{e}"))?
    {
        let count = bytes.len() as u64;
        if meta.lock().unwrap().bytes + count > RESPONSE_LIMIT {
            return Err("响应超过 128 MiB 限额，已停止接收".into());
        }
        if h.bytes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                v.checked_add(count).filter(|n| *n <= CACHE_LIMIT)
            })
            .is_err()
        {
            return Err("响应缓存已达 512 MiB，请清理响应或关闭标签".into());
        }
        let mut reservation = Reservation {
            bytes: &h.bytes,
            count,
            committed: false,
        };
        file.write_all(&bytes)
            .await
            .map_err(|e| format!("响应写盘失败：{e}"))?;
        let mut m = meta.lock().unwrap();
        m.bytes += count;
        reservation.committed = true;
    }
    file.flush().await.map_err(err)?;
    Ok(())
}

async fn read_limited(path: &str, max: u64) -> Result<Vec<u8>, String> {
    let file = tokio::fs::File::open(path).await.map_err(err)?;
    if file.metadata().await.map_err(err)?.len() > max {
        return Err("文件超过大小上限".into());
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(err)?;
    if bytes.len() as u64 > max {
        return Err("文件超过大小上限".into());
    }
    Ok(bytes)
}

#[tauri::command]
pub fn posttool_cancel(id: String) {
    let mut inner = hub().inner.lock().unwrap();
    if let Some(token) = inner.active.get(&id) {
        token.cancel();
    } else {
        inner.cancelled.push_back((id, Instant::now()));
        while inner.cancelled.len() > 32 {
            inner.cancelled.pop_front();
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resources {
    active: usize,
    responses: usize,
    clients: usize,
    cache_bytes: u64,
}
#[tauri::command]
pub fn posttool_resources() -> Resources {
    let Some(h) = HUB.get() else {
        return Resources {
            active: 0,
            responses: 0,
            clients: 0,
            cache_bytes: 0,
        };
    };
    let inner = h.inner.lock().unwrap();
    Resources {
        active: inner.active.len(),
        responses: inner.responses.len(),
        clients: inner.clients.len(),
        cache_bytes: h.bytes.load(Ordering::SeqCst),
    }
}
#[tauri::command]
pub fn posttool_release(id: Option<String>, clear_clients: bool) {
    let Some(h) = HUB.get() else { return };
    let mut inner = h.inner.lock().unwrap();
    let ids: Vec<_> = inner
        .order
        .iter()
        .filter(|x| id.as_ref().map(|v| v == *x).unwrap_or(true) && !inner.active.contains_key(*x))
        .cloned()
        .collect();
    for id in ids {
        forget(&mut inner, &id, h);
    }
    if clear_clients && inner.active.is_empty() {
        inner.clients.clear();
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slice {
    meta: ResponseMeta,
    offset: u64,
    next_offset: u64,
    data: Vec<u8>,
}
#[tauri::command]
pub async fn posttool_response_slice(id: String, offset: u64) -> Result<Slice, String> {
    let (path, mut meta, started) = {
        let inner = hub().inner.lock().unwrap();
        let entry = inner.responses.get(&id).ok_or("响应已释放，请重新发送")?;
        let m = entry.meta.lock().unwrap().clone();
        (entry.path.clone(), m, entry.started)
    };
    if meta.state == "connecting" || meta.state == "receiving" {
        meta.elapsed_ms = started.elapsed().as_millis() as u64;
    }
    let offset = offset.min(meta.bytes);
    let mut data = vec![0; (meta.bytes - offset).min(64 * 1024) as usize];
    if !data.is_empty() {
        let mut file = tokio::fs::File::open(path).await.map_err(err)?;
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(err)?;
        let n = file.read(&mut data).await.map_err(err)?;
        data.truncate(n);
    }
    Ok(Slice {
        meta,
        offset,
        next_offset: offset + data.len() as u64,
        data,
    })
}
#[tauri::command]
pub async fn posttool_response_save(id: String, path: String) -> Result<u64, String> {
    let source = {
        let inner = hub().inner.lock().unwrap();
        let entry = inner.responses.get(&id).ok_or("响应已释放")?;
        if inner.active.contains_key(&id) {
            return Err("请等待请求完成或取消后再保存".into());
        }
        entry.path.clone()
    };
    tokio::fs::copy(source, path).await.map_err(err)
}

#[cfg(test)]
mod tests;
