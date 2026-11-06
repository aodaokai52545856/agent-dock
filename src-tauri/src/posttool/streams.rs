//! Bounded native long-lived connections; no per-message UI events or unbounded channels.
use super::*;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest, protocol::WebSocketConfig, Message,
};

const QUEUE_BYTES: usize = 8 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connect {
    pub id: String,
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<Pair>,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub auth: String,
    #[serde(default)]
    pub clean_session: bool,
    #[serde(default)]
    pub proto_id: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub ca_path: String,
    #[serde(default)]
    pub identity_path: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub action: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub qos: u8,
    #[serde(default)]
    pub retain: bool,
    #[serde(default)]
    pub binary: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    seq: u64,
    time: i64,
    direction: String,
    topic: String,
    payload: String,
    binary: bool,
    truncated: bool,
}
pub(super) struct State {
    pub status: String,
    error: Option<String>,
    messages: VecDeque<MessageRow>,
    bytes: usize,
    seq: u64,
    dropped: u64,
}
pub(super) fn empty_state() -> Arc<Mutex<State>> {
    Arc::new(Mutex::new(State {
        status: "connecting".into(),
        error: None,
        messages: VecDeque::new(),
        bytes: 0,
        seq: 0,
        dropped: 0,
    }))
}
struct Session {
    token: CancellationToken,
    tx: mpsc::Sender<Command>,
    state: Arc<Mutex<State>>,
}
static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(super) fn push(
    state: &Arc<Mutex<State>>,
    direction: &str,
    topic: &str,
    payload: &str,
    binary: bool,
) {
    let mut state = state.lock().unwrap();
    let mut end = payload.len().min(64 * 1024);
    while !payload.is_char_boundary(end) {
        end -= 1
    }
    let truncated = end < payload.len();
    let payload = payload[..end].to_string();
    state.seq += 1;
    let row = MessageRow {
        seq: state.seq,
        time: chrono::Utc::now().timestamp_millis(),
        direction: direction.into(),
        topic: topic.into(),
        binary,
        payload,
        truncated,
    };
    state.bytes += row.payload.len() + row.topic.len();
    state.messages.push_back(row);
    while state.messages.len() > 5000 || state.bytes > QUEUE_BYTES {
        if let Some(row) = state.messages.pop_front() {
            state.bytes -= row.payload.len() + row.topic.len();
            state.dropped += 1
        } else {
            break;
        }
    }
}
pub fn shutdown() {
    if let Some(s) = SESSIONS.get() {
        for session in s.lock().unwrap().values() {
            session.token.cancel()
        }
    }
}
#[tauri::command]
pub fn posttool_stream_connect(spec: Connect) -> Result<(), String> {
    validate_id(&spec.id)?;
    if !matches!(
        spec.kind.as_str(),
        "websocket" | "socketio" | "mqtt" | "grpc"
    ) {
        return Err("不支持的长连接类型".into());
    }
    if spec.payload.len() > 1024 * 1024 || spec.headers.len() > 512 || spec.url.len() > 8192 {
        return Err("连接配置超过大小上限".into());
    }
    let mut all = sessions().lock().unwrap();
    if all.contains_key(&spec.id) {
        return Err("连接 ID 已存在".into());
    }
    let active = all
        .values()
        .filter(|s| !matches!(s.state.lock().unwrap().status.as_str(), "closed" | "error"))
        .count();
    if active >= 4 {
        return Err("同时最多 4 个长连接".into());
    }
    if all.len() >= 8 {
        let old = all
            .iter()
            .find(|(_, s)| matches!(s.state.lock().unwrap().status.as_str(), "closed" | "error"))
            .map(|(id, _)| id.clone());
        if let Some(id) = old {
            all.remove(&id);
        }
    }
    let (tx, rx) = mpsc::channel(16);
    let token = CancellationToken::new();
    let state = empty_state();
    all.insert(
        spec.id.clone(),
        Session {
            token: token.clone(),
            tx,
            state: state.clone(),
        },
    );
    tauri::async_runtime::spawn(async move {
        tls_provider();
        let result = tokio::select! {biased;_=token.cancelled()=>Ok(()),result=async{if spec.kind=="mqtt"{mqtt(&spec,rx,&state).await}else if spec.kind=="grpc"{super::grpc::run(&spec,rx,&state).await}else{websocket(&spec,rx,&state).await}}=>result};
        let mut state = state.lock().unwrap();
        state.status = if result.is_ok() { "closed" } else { "error" }.into();
        state.error = result.err();
    });
    Ok(())
}
#[tauri::command]
pub fn posttool_stream_send(id: String, command: Command) -> Result<(), String> {
    if command.payload.len() > 1024 * 1024 || command.topic.len() > 4096 {
        return Err("消息超过 1 MiB 或主题过长".into());
    }
    let all = sessions().lock().unwrap();
    let session = all.get(&id).ok_or("连接已关闭")?;
    session
        .tx
        .try_send(command)
        .map_err(|_| "发送队列已满或连接已结束，请稍后重试".into())
}
#[tauri::command]
pub fn posttool_stream_close(id: String) {
    if let Some(session) = sessions().lock().unwrap().get(&id) {
        session.token.cancel()
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    status: String,
    error: Option<String>,
    items: Vec<MessageRow>,
    cursor: u64,
    dropped: u64,
    buffer_bytes: usize,
}
#[tauri::command]
pub fn posttool_stream_poll(id: String, after: u64) -> Result<Batch, String> {
    let all = sessions().lock().unwrap();
    let s = all.get(&id).ok_or("连接缓存已释放")?.state.lock().unwrap();
    let mut bytes = 0;
    let items: Vec<_> = s
        .messages
        .iter()
        .filter(|m| m.seq > after)
        .take(100)
        .take_while(|m| {
            bytes += m.payload.len();
            bytes <= 64 * 1024
        })
        .cloned()
        .collect();
    let cursor = items.last().map(|m| m.seq).unwrap_or(after);
    Ok(Batch {
        status: s.status.clone(),
        error: s.error.clone(),
        items,
        cursor,
        dropped: s.dropped,
        buffer_bytes: s.bytes,
    })
}
#[tauri::command]
pub fn posttool_stream_release(id: String) {
    if let Some(session) = sessions().lock().unwrap().remove(&id) {
        session.token.cancel();
    }
}

async fn websocket(
    spec: &Connect,
    mut commands: mpsc::Receiver<Command>,
    state: &Arc<Mutex<State>>,
) -> Result<(), String> {
    let mut url = reqwest::Url::parse(&spec.url).map_err(err)?;
    let socketio = spec.kind == "socketio";
    if socketio {
        if url.scheme() == "http" {
            url.set_scheme("ws").map_err(|_| "URL 无效")?
        } else if url.scheme() == "https" {
            url.set_scheme("wss").map_err(|_| "URL 无效")?
        }
        if url.path() == "/" {
            url.set_path("/socket.io/")
        }
        url.query_pairs_mut()
            .append_pair("EIO", "4")
            .append_pair("transport", "websocket");
    }
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err("WebSocket 需要 ws:// 或 wss:// URL".into());
    }
    let mut request = url.as_str().into_client_request().map_err(err)?;
    for p in spec
        .headers
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
    {
        request.headers_mut().append(
            HeaderName::from_bytes(p.key.as_bytes()).map_err(err)?,
            HeaderValue::from_str(&p.value).map_err(err)?,
        );
    }
    let config = WebSocketConfig::default()
        .max_message_size(Some(1024 * 1024))
        .max_frame_size(Some(1024 * 1024))
        .max_write_buffer_size(2 * 1024 * 1024);
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        tokio_tungstenite::connect_async_with_config(request, Some(config), false),
    )
    .await
    .map_err(|_| "连接超时")?
    .map_err(err)?;
    if !socketio {
        state.lock().unwrap().status = "connected".into()
    }
    let ns = if spec.namespace.is_empty() || spec.namespace == "/" {
        String::new()
    } else {
        format!("{},", spec.namespace)
    };
    loop {
        tokio::select! {
            item=socket.next()=>{let Some(item)=item else{return Ok(())};match item.map_err(err)?{
                Message::Text(text)=>{
                    if socketio&&text.starts_with('0'){let auth=if spec.auth.trim().is_empty(){String::new()}else{serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&spec.auth).map_err(err)?).map_err(err)?};socket.send(Message::Text(format!("40{ns}{auth}").into())).await.map_err(err)?;}
                    else if socketio&&text=="2"{socket.send(Message::Text("3".into())).await.map_err(err)?;}
                    else {if socketio&&text.starts_with("40"){state.lock().unwrap().status="connected".into()}push(state,"in","",&text,false);}
                },
                Message::Binary(bytes)=>{use base64::Engine;push(state,"in","binary",&base64::engine::general_purpose::STANDARD.encode(bytes),true)},
                Message::Ping(_)=>{socket.flush().await.map_err(err)?;},Message::Close(_)=>return Ok(()),_=>{}
            }},
            command=commands.recv()=>{let Some(command)=command else{return Ok(())};
                if command.action=="ping"{socket.send(Message::Ping(Vec::new().into())).await.map_err(err)?;continue}
                let text=if socketio{if command.topic.is_empty(){return Err("Socket.IO 需要 event 名称".into())}let payload=serde_json::from_str::<serde_json::Value>(&command.payload).unwrap_or(serde_json::Value::String(command.payload.clone()));format!("42{ns}{}",serde_json::to_string(&serde_json::json!([command.topic,payload])).map_err(err)?)}else{command.payload.clone()};
                let message=if command.binary{use base64::Engine;Message::Binary(base64::engine::general_purpose::STANDARD.decode(&command.payload).map_err(err)?.into())}else{Message::Text(text.clone().into())};
                socket.send(message).await.map_err(err)?;push(state,"out",&command.topic,&text,command.binary);
            }
        }
    }
}
async fn mqtt(
    spec: &Connect,
    mut commands: mpsc::Receiver<Command>,
    state: &Arc<Mutex<State>>,
) -> Result<(), String> {
    use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, Transport};
    let url = reqwest::Url::parse(&spec.url).map_err(err)?;
    if !matches!(url.scheme(), "mqtt" | "mqtts") {
        return Err("MQTT 需要 mqtt:// 或 mqtts:// URL".into());
    }
    let host = url.host_str().ok_or("MQTT 缺少主机")?;
    let client_id = if spec.client_id.is_empty() {
        format!("posttool-{}", spec.id)
    } else {
        spec.client_id.clone()
    };
    let mut options = MqttOptions::new(
        client_id,
        host,
        url.port()
            .unwrap_or(if url.scheme() == "mqtts" { 8883 } else { 1883 }),
    );
    options.set_keep_alive(Duration::from_secs(30));
    options.set_clean_session(spec.clean_session);
    options.set_max_packet_size(1024 * 1024, 1024 * 1024);
    options.set_inflight(16);
    if !spec.username.is_empty() {
        options.set_credentials(&spec.username, &spec.password);
    }
    if url.scheme() == "mqtts" {
        options.set_transport(Transport::tls_with_default_config());
    }
    let (client, mut eventloop) = AsyncClient::new(options, 16);
    eventloop.network_options.set_connection_timeout(15);
    let qos = |n| match n {
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtMostOnce,
    };
    // Keep poll alive while commands arrive. Cancelling an in-progress connect/poll
    // for each command can close the transport before the MQTT handshake completes.
    let driver = async {
        loop {
            match eventloop.poll().await.map_err(err)? {
                Event::Incoming(Incoming::ConnAck(ack)) => {
                    if ack.code != rumqttc::ConnectReturnCode::Success {
                        return Err(format!("MQTT 拒绝连接：{:?}", ack.code));
                    }
                    state.lock().unwrap().status = "connected".into();
                }
                Event::Incoming(Incoming::Publish(p)) => push(
                    state,
                    "in",
                    &p.topic,
                    &String::from_utf8_lossy(&p.payload),
                    false,
                ),
                _ => {}
            }
        }
    };
    let control = async {
        let mut subscriptions = std::collections::HashSet::new();
        while let Some(command) = commands.recv().await {
            match command.action.as_str() {
                "subscribe" => {
                    if subscriptions.len() >= 100 {
                        return Err("最多 100 个 MQTT 订阅".into());
                    }
                    client
                        .subscribe(&command.topic, qos(command.qos))
                        .await
                        .map_err(err)?;
                    subscriptions.insert(command.topic.clone());
                    push(state, "system", &command.topic, "已提交订阅", false);
                }
                "unsubscribe" => {
                    client.unsubscribe(&command.topic).await.map_err(err)?;
                    subscriptions.remove(&command.topic);
                    push(state, "system", &command.topic, "已提交取消订阅", false);
                }
                "publish" => {
                    client
                        .publish(
                            &command.topic,
                            qos(command.qos),
                            command.retain,
                            command.payload.as_bytes(),
                        )
                        .await
                        .map_err(err)?;
                    push(state, "out", &command.topic, &command.payload, false);
                }
                _ => return Err("MQTT 操作无效".into()),
            }
        }
        Ok(())
    };
    tokio::select! {result=driver=>result,result=control=>result}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_ring_is_bounded() {
        let state = Arc::new(Mutex::new(State {
            status: "connected".into(),
            error: None,
            messages: VecDeque::new(),
            bytes: 0,
            seq: 0,
            dropped: 0,
        }));
        let data = "x".repeat(4096);
        for _ in 0..10_000 {
            push(&state, "in", "topic", &data, false);
        }
        let state = state.lock().unwrap();
        assert!(state.bytes <= QUEUE_BYTES);
        assert!(state.messages.len() <= 5000);
        assert!(state.dropped > 0);
    }
    fn state() -> Arc<Mutex<State>> {
        Arc::new(Mutex::new(State {
            status: "connecting".into(),
            error: None,
            messages: VecDeque::new(),
            bytes: 0,
            seq: 0,
            dropped: 0,
        }))
    }
    async fn mqtt_packet(socket: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
        let kind = socket.read_u8().await.unwrap();
        let mut size = 0;
        let mut multiplier = 1;
        loop {
            let byte = socket.read_u8().await.unwrap();
            size += ((byte & 127) as usize) * multiplier;
            if byte & 128 == 0 {
                break;
            }
            multiplier *= 128;
            assert!(multiplier <= 128 * 128 * 128);
        }
        assert!(size <= 1024 * 1024);
        let mut payload = vec![0; size];
        socket.read_exact(&mut payload).await.unwrap();
        (kind, payload)
    }
    #[tokio::test]
    async fn mqtt_connect_subscribe_publish_and_receive() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let spec = spec("mqtt", format!("mqtt://{address}"));
        let state = state();
        let (tx, rx) = mpsc::channel(16);
        let mut subscribe = command("");
        subscribe.action = "subscribe".into();
        tx.send(subscribe).await.unwrap();
        let mut publish = command("hello");
        publish.action = "publish".into();
        tx.send(publish).await.unwrap();
        let server = async {
            let (mut socket, _) = listener.accept().await.unwrap();
            let (kind, _) = mqtt_packet(&mut socket).await;
            assert_eq!(kind, 0x10);
            socket.write_all(&[0x20, 2, 0, 0]).await.unwrap();
            loop {
                let (kind, bytes) = mqtt_packet(&mut socket).await;
                if kind >> 4 == 8 {
                    socket
                        .write_all(&[0x90, 3, bytes[0], bytes[1], 0])
                        .await
                        .unwrap();
                } else if kind >> 4 == 3 {
                    assert!(bytes.ends_with(b"hello"));
                    socket
                        .write_all(&[0x30, 9, 0, 4, b'd', b'e', b'm', b'o', b'a', b'c', b'k'])
                        .await
                        .unwrap();
                    while !state
                        .lock()
                        .unwrap()
                        .messages
                        .iter()
                        .any(|m| m.direction == "in" && m.payload == "ack")
                    {
                        tokio::task::yield_now().await;
                    }
                    break;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            let _ = tokio::join!(mqtt(&spec, rx, &state), server);
        })
        .await
        .unwrap();
        assert!(state
            .lock()
            .unwrap()
            .messages
            .iter()
            .any(|m| m.direction == "in" && m.topic == "demo" && m.payload == "ack"));
    }
    fn spec(kind: &str, url: String) -> Connect {
        serde_json::from_value(
            serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"kind":kind,"url":url}),
        )
        .unwrap()
    }
    fn command(payload: &str) -> Command {
        Command {
            action: "send".into(),
            payload: payload.into(),
            topic: "demo".into(),
            qos: 0,
            retain: false,
            binary: false,
        }
    }
    #[tokio::test]
    async fn websocket_echo_and_peer_close() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let spec = spec("websocket", format!("ws://{address}"));
        let state = state();
        let (tx, rx) = mpsc::channel(16);
        tx.send(command("hello")).await.unwrap();
        let server = async {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            let message = ws.next().await.unwrap().unwrap();
            assert_eq!(message.to_text().unwrap(), "hello");
            ws.send(message).await.unwrap();
            ws.close(None).await.unwrap();
        };
        let (result, ()) = tokio::join!(websocket(&spec, rx, &state), server);
        result.unwrap();
        let state = state.lock().unwrap();
        assert!(state
            .messages
            .iter()
            .any(|m| m.direction == "in" && m.payload == "hello"));
    }
    #[tokio::test]
    async fn socketio_engine_handshake_and_event() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let spec = spec("socketio", format!("http://{address}"));
        let state = state();
        let (tx, rx) = mpsc::channel(16);
        tx.send(command("{\"ok\":true}")).await.unwrap();
        let server = async {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            ws.send(Message::Text(
                "0{\"sid\":\"test\",\"pingInterval\":25000,\"pingTimeout\":20000}".into(),
            ))
            .await
            .unwrap();
            let first = ws.next().await.unwrap().unwrap();
            let text = first.to_text().unwrap();
            if text.starts_with("42") {
                assert!(text.contains("demo"));
                let second = ws.next().await.unwrap().unwrap();
                assert_eq!(second.to_text().unwrap(), "40")
            } else {
                assert_eq!(text, "40");
                let second = ws.next().await.unwrap().unwrap();
                assert!(second.to_text().unwrap().starts_with("42"))
            }
            ws.send(Message::Text("40{\"sid\":\"socket\"}".into()))
                .await
                .unwrap();
            ws.send(Message::Text("42[\"reply\",true]".into()))
                .await
                .unwrap();
            ws.close(None).await.unwrap();
        };
        let (result, ()) = tokio::join!(websocket(&spec, rx, &state), server);
        result.unwrap();
        assert!(state
            .lock()
            .unwrap()
            .messages
            .iter()
            .any(|m| m.payload.contains("reply")));
    }
}
