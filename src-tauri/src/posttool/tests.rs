use super::*;

#[test]
fn ids_cannot_escape_cache_directory() {
    assert!(validate_id("../response").is_err());
    assert!(validate_id(&uuid::Uuid::new_v4().to_string()).is_ok());
}

#[tokio::test]
async fn dropping_cancelled_io_releases_socket() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let token = CancellationToken::new();
    let client = Client::builder().no_proxy().build().unwrap();
    let operation = async {
        tokio::select! { biased; _ = token.cancelled() => false, result = client.get(format!("http://{address}")).send() => { result.unwrap(); true } }
    };
    let server = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        token.cancel();
        let bytes = tokio::time::timeout(Duration::from_secs(1), socket.read(&mut request))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bytes, 0);
    };
    let (completed, ()) = tokio::join!(operation, server);
    assert!(!completed);
}

#[test]
fn cache_and_client_limits_are_bounded() {
    assert_eq!(MAX_RESPONSES, 24);
    let hub = isolated_hub();
    let id = uuid::Uuid::new_v4().to_string();
    hub.inner
        .lock()
        .unwrap()
        .active
        .insert(id.clone(), CancellationToken::new());
    drop(ActiveGuard {
        id,
        hub: hub.clone(),
    });
    assert!(hub.inner.lock().unwrap().active.is_empty());
}

fn isolated_hub() -> Arc<Hub> {
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
}
fn spec(url: String) -> RequestSpec {
    serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"method":"GET","url":url,"proxy":"direct"})).unwrap()
}

#[test]
fn cookies_are_bounded_and_survive_client_cache_release() {
    use reqwest::cookie::CookieStore as _;
    let h = isolated_hub();
    let url = reqwest::Url::parse("https://example.test/path").unwrap();
    let cookie = HeaderValue::from_static("session=sample; Path=/; HttpOnly");
    h.cookies.set_cookies(&mut std::iter::once(&cookie), &url);
    h.inner.lock().unwrap().clients.clear();
    assert_eq!(
        h.cookies.cookies(&url).unwrap().to_str().unwrap(),
        "session=sample"
    );
    assert!(h
        .cookies
        .cookies(&reqwest::Url::parse("https://another.test/").unwrap())
        .is_none());
    for index in 0..1000 {
        let header = HeaderValue::from_str(&format!("cookie{index}=value; Path=/")).unwrap();
        h.cookies.set_cookies(&mut std::iter::once(&header), &url);
    }
    assert!(h.cookies.0.lock().unwrap().iter_any().count() <= 512);
}

#[tokio::test]
async fn hundred_megabyte_response_streams_to_disk_without_body_in_metadata() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.body");
    let request = spec(format!("http://{address}/large"));
    let h = isolated_hub();
    let meta = Arc::new(Mutex::new(ResponseMeta {
        id: request.id.clone(),
        ..Default::default()
    }));
    let server = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 4096];
        socket.read(&mut buf).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 104857600\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n").await.unwrap();
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..1600 {
            socket.write_all(&chunk).await.unwrap();
        }
    };
    let download = execute(&request, &path, &meta, &h);
    let (result, ()) = tokio::join!(download, server);
    result.unwrap();
    assert_eq!(std::fs::metadata(path).unwrap().len(), 100 * 1024 * 1024);
    assert_eq!(h.bytes.load(Ordering::SeqCst), 100 * 1024 * 1024);
    assert!(serde_json::to_vec(&*meta.lock().unwrap()).unwrap().len() < 2048);
    assert_eq!(meta.lock().unwrap().bytes, 100 * 1024 * 1024);
}

#[tokio::test]
async fn server_errors_keep_body_and_duplicate_headers() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("error.body");
    let request = spec(format!("http://{address}"));
    let h = isolated_hub();
    let meta = Arc::new(Mutex::new(ResponseMeta::default()));
    let server = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 4096];
        socket.read(&mut buf).await.unwrap();
        socket.write_all(b"HTTP/1.1 422 Unprocessable Entity\r\nX-Repeat: one\r\nX-Repeat: two\r\nContent-Length: 7\r\nConnection: close\r\n\r\ninvalid").await.unwrap();
    };
    let (result, ()) = tokio::join!(execute(&request, &path, &meta, &h), server);
    result.unwrap();
    assert_eq!(meta.lock().unwrap().status, 422);
    assert_eq!(
        meta.lock()
            .unwrap()
            .headers
            .iter()
            .filter(|p| p.key == "x-repeat")
            .count(),
        2
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid");
}

#[test]
fn reservation_rolls_back_on_cancelled_write() {
    let bytes = AtomicU64::new(100);
    {
        let _reservation = Reservation {
            bytes: &bytes,
            count: 100,
            committed: false,
        };
    }
    assert_eq!(bytes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn thousand_requests_reclaim_response_files_tasks_and_accounting() {
    const COUNT: usize = 1000;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let h = isolated_hub();
    let server = async {
        for _ in 0..COUNT {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            socket.read(&mut bytes).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        }
    };
    let requests = async {
        for index in 0..COUNT {
            let mut request = spec(format!("http://{address}/{index}"));
            request.cookies = index % 2 == 0;
            request.follow_redirects = index % 3 == 0;
            request.insecure = index % 5 == 0;
            let path = dir.path().join(format!("{}.body", request.id));
            let meta = Arc::new(Mutex::new(ResponseMeta::default()));
            {
                let mut inner = h.inner.lock().unwrap();
                inner
                    .active
                    .insert(request.id.clone(), CancellationToken::new());
                inner.responses.insert(
                    request.id.clone(),
                    ResponseEntry {
                        meta: meta.clone(),
                        path: path.clone(),
                        started: Instant::now(),
                    },
                );
                inner.order.push_back(request.id.clone());
            }
            let guard = ActiveGuard {
                id: request.id.clone(),
                hub: h.clone(),
            };
            execute(&request, &path, &meta, &h).await.unwrap();
            drop(guard);
            let mut inner = h.inner.lock().unwrap();
            forget(&mut inner, &request.id, &h);
            assert!(inner.active.is_empty());
            assert!(inner.responses.is_empty());
            assert!(inner.clients.len() <= 4);
            assert_eq!(h.bytes.load(Ordering::SeqCst), 0);
        }
    };
    tokio::time::timeout(Duration::from_secs(30), async {
        tokio::join!(server, requests);
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn abandoned_cache_is_reaped_without_touching_live_instance() {
    let dir = tempfile::tempdir().unwrap();
    let first = isolated_hub();
    let first_path = cache_directory(dir.path(), &first).unwrap();
    std::fs::write(first_path.join("one.body"), b"payload").unwrap();
    let second = isolated_hub();
    cache_directory(dir.path(), &second).unwrap();
    assert!(first_path.join("one.body").exists());
    drop(first);
    let third = isolated_hub();
    cache_directory(dir.path(), &third).unwrap();
    assert!(!first_path.exists());
}
