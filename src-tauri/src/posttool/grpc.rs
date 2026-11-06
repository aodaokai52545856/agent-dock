use super::streams::{push, Command, Connect, State as StreamState};
use super::*;
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity},
    Status,
};
static POOLS: OnceLock<Mutex<VecDeque<(String, DescriptorPool)>>> = OnceLock::new();
fn pools() -> &'static Mutex<VecDeque<(String, DescriptorPool)>> {
    POOLS.get_or_init(|| Mutex::new(VecDeque::new()))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodInfo {
    name: String,
    service: String,
    client_streaming: bool,
    server_streaming: bool,
    input: String,
    output: String,
}
#[derive(Serialize)]
pub struct ProtoInfo {
    id: String,
    methods: Vec<MethodInfo>,
}
fn register(pool: DescriptorPool) -> ProtoInfo {
    let id = uuid::Uuid::new_v4().to_string();
    let methods = pool
        .services()
        .flat_map(|s| {
            s.methods()
                .map(|m| MethodInfo {
                    name: format!("{}/{}", s.full_name(), m.name()),
                    service: s.full_name().into(),
                    client_streaming: m.is_client_streaming(),
                    server_streaming: m.is_server_streaming(),
                    input: m.input().full_name().into(),
                    output: m.output().full_name().into(),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut pools = pools().lock().unwrap();
    pools.push_back((id.clone(), pool));
    while pools.len() > 4 {
        pools.pop_front();
    }
    ProtoInfo { id, methods }
}

#[tauri::command]
pub async fn posttool_proto_load(path: String, includes: Vec<String>) -> Result<ProtoInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        let mut roots: Vec<PathBuf> = includes.into_iter().map(PathBuf::from).collect();
        roots.push(path.parent().ok_or("proto 路径无效")?.to_path_buf());
        let mut seen = std::collections::HashSet::new();
        let mut total = 0;
        let mut count = 0;
        let mut pending: Vec<_> = roots.iter().cloned().map(|p| (p, 0)).collect();
        while let Some((dir, depth)) = pending.pop() {
            let dir = dir.canonicalize().map_err(err)?;
            if !seen.insert(dir.clone()) {
                continue;
            }
            if depth > 16 {
                return Err("proto 依赖目录超过 16 层".into());
            }
            for file in std::fs::read_dir(dir).map_err(err)? {
                let file = file.map_err(err)?;
                let ty = file.file_type().map_err(err)?;
                if ty.is_dir() {
                    pending.push((file.path(), depth + 1));
                } else if ty.is_file() && file.path().extension().is_some_and(|e| e == "proto") {
                    let size = file.metadata().map_err(err)?.len();
                    count += 1;
                    total += size;
                    if size > 2 * 1024 * 1024 || total > 16 * 1024 * 1024 || count > 512 {
                        return Err(
                            "proto 目录超过 512 文件 / 16 MiB 上限，请选择更小的依赖目录".into(),
                        );
                    }
                }
            }
        }
        let set = protox::compile([path], roots).map_err(err)?;
        if set.encoded_len() > 16 * 1024 * 1024 {
            return Err("proto 描述符超过 16 MiB".into());
        }
        Ok(register(
            DescriptorPool::from_file_descriptor_set(set).map_err(err)?,
        ))
    })
    .await
    .map_err(err)?
}
async fn channel(url: &str, ca: &str, identity: &str) -> Result<Channel, String> {
    tls_provider();
    let mut endpoint = Endpoint::from_shared(url.to_string())
        .map_err(err)?
        .connect_timeout(Duration::from_secs(15));
    if url.starts_with("https://") {
        let mut tls = ClientTlsConfig::new().with_native_roots();
        if !ca.is_empty() {
            tls = tls.ca_certificate(Certificate::from_pem(read_limited(ca, 1024 * 1024).await?));
        }
        if !identity.is_empty() {
            let pem = read_limited(identity, 1024 * 1024).await?;
            tls = tls.identity(Identity::from_pem(pem.clone(), pem));
        }
        endpoint = endpoint.tls_config(tls).map_err(err)?;
    }
    endpoint.connect().await.map_err(err)
}
fn metadata<T>(input: T, headers: &[Pair]) -> Result<tonic::Request<T>, String> {
    let mut request = tonic::Request::new(input);
    for p in headers.iter().filter(|p| p.enabled && !p.key.is_empty()) {
        if p.key.ends_with("-bin") {
            use base64::Engine;
            let key = tonic::metadata::MetadataKey::<tonic::metadata::Binary>::from_bytes(
                p.key.as_bytes(),
            )
            .map_err(err)?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&p.value)
                .map_err(err)?;
            request
                .metadata_mut()
                .append_bin(key, tonic::metadata::MetadataValue::from_bytes(&bytes));
        } else {
            let key = tonic::metadata::MetadataKey::<tonic::metadata::Ascii>::from_bytes(
                p.key.as_bytes(),
            )
            .map_err(err)?;
            request
                .metadata_mut()
                .append(key, p.value.parse().map_err(err)?);
        }
    }
    Ok(request)
}

#[tauri::command]
pub async fn posttool_grpc_reflect(
    url: String,
    headers: Vec<Pair>,
    symbol: Option<String>,
) -> Result<serde_json::Value, String> {
    tokio::time::timeout(Duration::from_secs(20),async{
        use tonic_reflection::pb::v1alpha::{server_reflection_client::ServerReflectionClient,ServerReflectionRequest,server_reflection_request::MessageRequest,server_reflection_response::MessageResponse};
        let request=ServerReflectionRequest{host:String::new(),message_request:Some(match symbol{Some(symbol)=>MessageRequest::FileContainingSymbol(symbol),None=>MessageRequest::ListServices(String::new())})};
        let mut client=ServerReflectionClient::new(channel(&url,"","").await?).max_decoding_message_size(16*1024*1024);
        let mut responses=client.server_reflection_info(metadata(tokio_stream::iter([request]),&headers)?).await.map_err(err)?.into_inner();
        match responses.message().await.map_err(err)?.and_then(|r|r.message_response){
            Some(MessageResponse::ListServicesResponse(list))=>Ok(serde_json::json!({"services":list.service.into_iter().map(|s|s.name).collect::<Vec<_>>()})),
            Some(MessageResponse::FileDescriptorResponse(files))=>{if files.file_descriptor_proto.iter().map(|b|b.len()).sum::<usize>()>16*1024*1024{return Err("描述符超过 16 MiB".into())}let set=prost_types::FileDescriptorSet{file:files.file_descriptor_proto.into_iter().map(|b|prost_types::FileDescriptorProto::decode(b.as_slice())).collect::<Result<Vec<_>,_>>().map_err(err)?};let info=register(DescriptorPool::from_file_descriptor_set(set).map_err(err)?);serde_json::to_value(info).map_err(err)},
            Some(MessageResponse::ErrorResponse(error))=>Err(format!("Reflection {}：{}",error.error_code,error.error_message)),_=>Err("服务未返回 Reflection 数据".into())
        }
    }).await.map_err(|_|"Reflection 超时".to_string())?
}
#[derive(Clone)]
struct DynamicCodec {
    output: MessageDescriptor,
}
struct DynamicEncoder;
struct DynamicDecoder {
    descriptor: MessageDescriptor,
}
impl Codec for DynamicCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicEncoder;
    type Decoder = DynamicDecoder;
    fn encoder(&mut self) -> Self::Encoder {
        DynamicEncoder
    }
    fn decoder(&mut self) -> Self::Decoder {
        DynamicDecoder {
            descriptor: self.output.clone(),
        }
    }
}
impl Encoder for DynamicEncoder {
    type Item = DynamicMessage;
    type Error = Status;
    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Status> {
        item.encode(dst)
            .map_err(|e| Status::invalid_argument(e.to_string()))
    }
}
impl Decoder for DynamicDecoder {
    type Item = DynamicMessage;
    type Error = Status;
    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Status> {
        DynamicMessage::decode(self.descriptor.clone(), src)
            .map(Some)
            .map_err(|e| Status::data_loss(e.to_string()))
    }
}
fn parse(descriptor: MessageDescriptor, text: &str) -> Result<DynamicMessage, String> {
    if text.len() > 1024 * 1024 {
        return Err("gRPC 请求消息超过 1 MiB".into());
    }
    let mut json =
        serde_json::Deserializer::from_str(if text.trim().is_empty() { "{}" } else { text });
    let message = DynamicMessage::deserialize(descriptor, &mut json).map_err(err)?;
    json.end().map_err(err)?;
    Ok(message)
}
pub(super) async fn run(
    spec: &Connect,
    mut commands: mpsc::Receiver<Command>,
    state: &Arc<Mutex<StreamState>>,
) -> Result<(), String> {
    let pool = pools()
        .lock()
        .unwrap()
        .iter()
        .find(|(id, _)| id == &spec.proto_id)
        .map(|(_, p)| p.clone())
        .ok_or("proto 描述符已释放，请重新加载")?;
    let (service, method) = spec.method.rsplit_once('/').ok_or("请选择 gRPC 方法")?;
    let descriptor = pool
        .get_service_by_name(service)
        .and_then(|s| s.methods().find(|m| m.name() == method))
        .ok_or("找不到 gRPC 方法")?;
    let input = descriptor.input();
    let output = descriptor.output();
    let client_streaming = descriptor.is_client_streaming();
    let first = parse(input.clone(), &spec.payload)?;
    let channel = channel(&spec.url, &spec.ca_path, &spec.identity_path).await?;
    let mut client = tonic::client::Grpc::new(channel)
        .max_decoding_message_size(1024 * 1024)
        .max_encoding_message_size(1024 * 1024);
    client.ready().await.map_err(err)?;
    let (tx, rx) = mpsc::channel(16);
    tx.send(first).await.map_err(err)?;
    let requests = ReceiverStream::new(rx);
    let request = metadata(requests, &spec.headers)?;
    let path = format!("/{service}/{method}")
        .parse::<tonic::codegen::http::uri::PathAndQuery>()
        .map_err(err)?;
    let mut sender = if client_streaming {
        Some(tx)
    } else {
        drop(tx);
        None
    };
    state.lock().unwrap().status = "connected".into();
    let response = {
        let response = client.streaming(request, path, DynamicCodec { output });
        tokio::pin!(response);
        loop {
            tokio::select! {
                result=&mut response=>break result.map_err(err)?,
                command=commands.recv()=>{let command=command.ok_or("调用已结束")?;send_command(command,&mut sender,&input,state,method)?;}
            }
        }
    };
    push(
        state,
        "system",
        "metadata",
        &format!("{:?}", response.metadata()),
        false,
    );
    state.lock().unwrap().status = "connected".into();
    let mut stream = response.into_inner();
    loop {
        tokio::select! {
            item=stream.message()=>{match item.map_err(|e|format!("gRPC {:?}：{}",e.code(),e.message()))?{Some(message)=>push(state,"in",method,&serde_json::to_string(&message).map_err(err)?,false),None=>{if let Some(trailers)=stream.trailers().await.map_err(err)?{push(state,"system","trailers",&format!("{trailers:?}"),false)}return Ok(())}}},
            command=commands.recv()=>{let Some(command)=command else{return Ok(())};send_command(command,&mut sender,&input,state,method)?;}
        }
    }
}
fn send_command(
    command: Command,
    sender: &mut Option<mpsc::Sender<DynamicMessage>>,
    input: &MessageDescriptor,
    state: &Arc<Mutex<StreamState>>,
    method: &str,
) -> Result<(), String> {
    if command.action == "finish" {
        sender.take();
        push(state, "system", "", "客户端发送流已结束", false);
    } else if let Some(sender) = sender.as_ref() {
        let message = parse(input.clone(), &command.payload)?;
        sender.try_send(message).map_err(|_| "gRPC 发送队列已满")?;
        push(state, "out", method, &command.payload, false);
    } else {
        return Err("此方法不支持继续发送消息，或发送流已经结束".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{Stream, StreamExt};
    use std::{
        convert::Infallible,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };
    use tonic::codegen::http;
    type Replies = Pin<Box<dyn Stream<Item = Result<DynamicMessage, Status>> + Send>>;
    #[derive(Clone)]
    struct EchoRouter {
        descriptor: MessageDescriptor,
    }
    struct EchoService {
        mode: String,
    }
    impl tonic::server::StreamingService<DynamicMessage> for EchoService {
        type Response = DynamicMessage;
        type ResponseStream = Replies;
        type Future =
            Pin<Box<dyn Future<Output = Result<tonic::Response<Replies>, Status>> + Send>>;
        fn call(
            &mut self,
            request: tonic::Request<tonic::Streaming<DynamicMessage>>,
        ) -> Self::Future {
            let mode = self.mode.clone();
            Box::pin(async move {
                let mut input = request.into_inner();
                let replies: Replies = if mode == "Bidi" {
                    Box::pin(input.map(|m| m))
                } else {
                    let mut values = Vec::new();
                    while let Some(message) = input.message().await? {
                        values.push(message)
                    }
                    let message = values
                        .pop()
                        .ok_or(Status::invalid_argument("missing message"))?;
                    if mode == "Client" {
                        assert_eq!(values.len(), 1)
                    }
                    let output = if mode == "Server" {
                        vec![Ok(message.clone()), Ok(message)]
                    } else {
                        vec![Ok(message)]
                    };
                    Box::pin(tokio_stream::iter(output))
                };
                Ok(tonic::Response::new(replies))
            })
        }
    }
    impl tonic::server::NamedService for EchoRouter {
        const NAME: &'static str = "test.Echo";
    }
    impl tonic::codegen::Service<http::Request<tonic::body::Body>> for EchoRouter {
        type Response = http::Response<tonic::body::Body>;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
            let mode = request.uri().path().rsplit('/').next().unwrap().to_string();
            let codec = DynamicCodec {
                output: self.descriptor.clone(),
            };
            Box::pin(async move {
                let mut grpc = tonic::server::Grpc::new(codec);
                Ok(grpc.streaming(EchoService { mode }, request).await)
            })
        }
    }
    const SOURCE:&str="syntax = \"proto3\"; package test; message M { string value=1; } service Echo {rpc Unary(M) returns(M); rpc Server(M) returns(stream M); rpc Client(stream M) returns(M); rpc Bidi(stream M) returns(stream M);}";
    fn pool() -> DescriptorPool {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("echo.proto");
        std::fs::write(&path, SOURCE).unwrap();
        DescriptorPool::from_file_descriptor_set(protox::compile([path], [dir.path()]).unwrap())
            .unwrap()
    }
    #[test]
    fn dynamic_json_roundtrip_and_streaming_flags() {
        let pool = pool();
        let message = pool.get_message_by_name("test.M").unwrap();
        let dynamic = parse(message.clone(), "{\"value\":\"你好\"}").unwrap();
        let bytes = dynamic.encode_to_vec();
        let decoded = DynamicMessage::decode(message, bytes.as_slice()).unwrap();
        assert_eq!(
            serde_json::to_string(&decoded).unwrap(),
            "{\"value\":\"你好\"}"
        );
        let info = register(pool);
        assert_eq!(info.methods.len(), 4);
        assert_eq!(
            info.methods.iter().filter(|m| m.client_streaming).count(),
            2
        );
        assert_eq!(
            info.methods.iter().filter(|m| m.server_streaming).count(),
            2
        );
    }
    #[tokio::test]
    async fn all_four_patterns_use_real_http2_and_client_stream_half_close() {
        let pool = pool();
        let router = EchoRouter {
            descriptor: pool.get_message_by_name("test.M").unwrap(),
        };
        let info = register(pool);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = CancellationToken::new();
        let server_token = shutdown.clone();
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(router)
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    server_token.cancelled_owned(),
                )
                .await
                .unwrap()
        });
        for method in ["Unary", "Server", "Client", "Bidi"] {
            let spec:Connect=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"kind":"grpc","url":format!("http://{address}"),"protoId":info.id,"method":format!("test.Echo/{method}"),"payload":"{\"value\":\"first\"}"})).unwrap();
            let (tx, rx) = mpsc::channel(16);
            if matches!(method, "Client" | "Bidi") {
                tx.send(Command {
                    action: "send".into(),
                    payload: "{\"value\":\"second\"}".into(),
                    topic: String::new(),
                    qos: 0,
                    retain: false,
                    binary: false,
                })
                .await
                .unwrap();
                tx.send(Command {
                    action: "finish".into(),
                    payload: String::new(),
                    topic: String::new(),
                    qos: 0,
                    retain: false,
                    binary: false,
                })
                .await
                .unwrap();
            }
            let state = super::super::streams::empty_state();
            tokio::time::timeout(Duration::from_secs(5), run(&spec, rx, &state))
                .await
                .unwrap()
                .unwrap();
            drop(tx);
        }
        shutdown.cancel();
        server.await.unwrap();
    }
}
