//! AI handoff bundles: bodies stay on disk, clipboard contains only the entry point.
use super::*;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufWriter, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyItem {
    request: Value,
    #[serde(default)]
    response_id: Option<String>,
    #[serde(default)]
    resolved: Option<RequestSpec>,
    #[serde(default)]
    resolution_error: Option<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    tests: Vec<Value>,
    #[serde(default)]
    logs: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    directory: String,
    count: usize,
    clipboard_text: String,
    clipboard_error: Option<String>,
}

fn application_directory(executable: &Path) -> Result<PathBuf, String> {
    let parent = executable.parent().ok_or("无法确定软件所在目录")?;
    // On macOS, put tmp next to the .app bundle rather than inside its signed contents.
    if parent.file_name().is_some_and(|n| n == "MacOS") {
        if let Some(contents) = parent
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == "Contents"))
        {
            if let Some(bundle) = contents
                .parent()
                .filter(|p| p.extension().is_some_and(|n| n == "app"))
            {
                return bundle
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or("无法确定软件所在目录".into());
            }
        }
    }
    Ok(parent.to_path_buf())
}

fn visible_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    // Windows APIs may return extended paths; clipboard paths should also work in AI tools/shells.
    #[cfg(windows)]
    {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(local) = value.strip_prefix(r"\\?\") {
            return local.to_string();
        }
    }
    value.into_owned()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut writer = BufWriter::new(File::create(path).map_err(err)?);
    serde_json::to_writer_pretty(&mut writer, value).map_err(err)?;
    writer.flush().map_err(err)
}

fn slug(name: &str) -> String {
    let name: String = name
        .chars()
        .take(48)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "request".into()
    } else {
        name
    }
}

fn body_extension(mode: &str) -> &'static str {
    match mode {
        "json" => "json",
        "graphql" => "graphql",
        "xml" => "xml",
        "html" => "html",
        _ => "txt",
    }
}

fn response_extension(content_type: &str) -> &'static str {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime == "application/json" || mime.ends_with("+json") {
        "json"
    } else if mime == "text/html" {
        "html"
    } else if mime.ends_with("/xml") || mime.ends_with("+xml") {
        "xml"
    } else if mime.starts_with("text/") || mime == "application/javascript" {
        "txt"
    } else {
        "bin"
    }
}

fn write_request(dir: &Path, prefix: &str, mut request: Value) -> Result<Value, String> {
    let object = request.as_object_mut().ok_or("请求数据不是有效对象")?;
    let mut mode = object
        .get("bodyMode")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if prefix != "request" && mode == "graphql" {
        mode = "json".into();
    }
    for (key, suffix, extension) in [
        ("body", "body", body_extension(&mode)),
        ("graphqlVariables", "graphql-variables", "json"),
        ("preScript", "pre-script", "js"),
        ("postScript", "post-script", "js"),
    ] {
        if let Some(Value::String(text)) = object.remove(key) {
            let filename = format!("{prefix}-{suffix}.{extension}");
            std::fs::write(dir.join(&filename), text).map_err(err)?;
            object.insert(format!("{key}File"), Value::String(filename));
        }
    }
    write_json(&dir.join(format!("{prefix}.json")), &request)?;
    Ok(request)
}

fn copy_attachments(dir: &Path, request: &Value) -> Result<Vec<Value>, String> {
    let mut sources = Vec::new();
    if request["bodyMode"] == "binary" {
        if let Some(path) = request["filePath"].as_str().filter(|p| !p.is_empty()) {
            sources.push(("body".to_string(), path.to_string()));
        }
    }
    if request["bodyMode"] == "multipart" {
        if let Some(fields) = request["fields"].as_array() {
            for field in fields {
                if field["file"] == true
                    && field["enabled"] != false
                    && field["key"].as_str().is_some_and(|key| !key.is_empty())
                {
                    if let Some(path) = field["value"].as_str().filter(|p| !p.is_empty()) {
                        sources.push((
                            field["key"].as_str().unwrap_or("").to_string(),
                            path.to_string(),
                        ));
                    }
                }
            }
        }
    }
    let mut result = Vec::new();
    for (index, (field, source)) in sources.into_iter().enumerate() {
        let filename = format!("attachments/{:03}-{}.bin", index + 1, slug(&field));
        std::fs::create_dir_all(dir.join("attachments")).map_err(err)?;
        // fs::copy does not load large uploads into memory.
        match std::fs::copy(&source, dir.join(&filename)) {
            Ok(bytes) => {
                result.push(json!({"field":field,"source":source,"file":filename,"bytes":bytes}))
            }
            Err(e) => result
                .push(json!({"field":field,"source":source,"file":null,"error":e.to_string()})),
        }
    }
    Ok(result)
}

fn write_item(dir: &Path, item: CopyItem, h: &Hub) -> Result<Value, String> {
    std::fs::create_dir(dir).map_err(err)?;
    let name = item.request["name"]
        .as_str()
        .unwrap_or("未命名请求")
        .to_owned();
    let method = item.request["method"].as_str().unwrap_or("").to_owned();
    let url = item.request["url"].as_str().unwrap_or("").to_owned();
    let request = write_request(dir, "request", item.request)?;
    let mut notes = Vec::new();
    if item.source == "history" {
        notes.push("历史中的原始请求已按既有规则脱敏并省略正文；如响应缓存仍在，sent-request.json 包含最近发送快照。".to_string());
    }
    if let Some(error) = item.resolution_error {
        notes.push(format!(
            "当前配置未能解析变量/鉴权，原始配置已保留：{error}"
        ));
    }
    let mut attachment_request = request;
    let mut response = json!({"available":false,"reason":"此请求尚未发送或没有可用的响应缓存"});
    if let Some(id) = item.response_id {
        // Open under the lock so releasing/evicting the cache cannot swap the source.
        let captured = {
            let inner = h.inner.lock().unwrap();
            if inner.active.contains_key(&id) {
                return Err(format!("{name} 正在接收响应，请完成或停止请求后再复制"));
            }
            inner.responses.get(&id).map(|entry| {
                let meta = entry.meta.lock().unwrap().clone();
                (entry.request.clone(), meta, File::open(&entry.path))
            })
        };
        if let Some((sent, meta, body)) = captured {
            attachment_request = write_request(
                dir,
                "sent-request",
                serde_json::to_value(sent).map_err(err)?,
            )?;
            let filename = format!("response-body.{}", response_extension(&meta.content_type));
            let body_result = match body {
                Ok(mut source) => File::create(dir.join(&filename))
                    .and_then(|mut destination| std::io::copy(&mut source, &mut destination)),
                Err(_) if meta.bytes == 0 => File::create(dir.join(&filename)).map(|_| 0),
                Err(e) => Err(e),
            };
            let mut metadata = serde_json::to_value(&meta).map_err(err)?;
            match body_result {
                Ok(bytes) => {
                    metadata["bodyFile"] = json!(filename);
                    metadata["capturedBytes"] = json!(bytes);
                    response = json!({"available":true,"metadataFile":"response.json","bodyFile":filename,"state":meta.state,"bytes":bytes});
                }
                Err(e) => {
                    metadata["bodyError"] = json!(e.to_string());
                    notes.push(format!("响应正文无法复制：{e}"));
                    response = json!({"available":false,"metadataFile":"response.json","reason":e.to_string()});
                }
            }
            if meta.state != "done" {
                notes.push(
                    "最近发送未正常完成；响应正文只包含已接收的内容，状态与错误见 response.json。"
                        .into(),
                );
            }
            write_json(&dir.join("response.json"), &metadata)?;
        } else {
            notes.push(
                "最近响应缓存已释放，无法恢复该次发送快照或响应正文；需要重新发送后复制。".into(),
            );
            response = json!({"available":false,"reason":"响应缓存已释放，请重新发送后复制"});
        }
    }
    if let Some(resolved) = item.resolved {
        let resolved = write_request(
            dir,
            "resolved-request",
            serde_json::to_value(resolved).map_err(err)?,
        )?;
        if !dir.join("sent-request.json").exists() {
            attachment_request = resolved;
        }
    }
    let attachments = copy_attachments(dir, &attachment_request)?;
    write_json(
        &dir.join("diagnostics.json"),
        &json!({"error":item.error,"tests":item.tests,"logs":item.logs}),
    )?;
    let summary = json!({"name":name,"method":method,"url":url,"source":item.source,"requestFile":"request.json","response":response,"attachments":attachments,"notes":notes});
    write_json(&dir.join("index.json"), &summary)?;
    Ok(summary)
}

fn create_bundle(base: &Path, items: Vec<CopyItem>, h: &Hub) -> Result<CopyResult, String> {
    if items.is_empty() || items.len() > 100 {
        return Err("请选择 1–100 条请求进行复制".into());
    }
    create_bundle_from_source(base, h, |_, visitor| {
        for item in items {
            visitor(item)?;
        }
        Ok(())
    })
}

fn create_bundle_from_source(
    base: &Path,
    h: &Hub,
    source: impl FnOnce(&Path, &mut dyn FnMut(CopyItem) -> Result<(), String>) -> Result<(), String>,
) -> Result<CopyResult, String> {
    let tmp = base.join("tmp");
    std::fs::create_dir_all(&tmp)
        .map_err(|e| format!("无法在软件目录创建 {}：{e}", tmp.display()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = tmp.join(format!(
        "posttool-{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        &id[..8]
    ));
    let staging = tmp.join(format!(".posttool-{id}.partial"));
    std::fs::create_dir(&staging).map_err(err)?;
    let result = (|| {
        let mut entries = Vec::new();
        source(&staging, &mut |item| {
            let index = entries.len();
            let folder = format!(
                "{:03}-{}",
                index + 1,
                slug(item.request["name"].as_str().unwrap_or("request"))
            );
            let mut summary = write_item(&staging.join(&folder), item, h)?;
            summary["directory"] = json!(folder);
            entries.push(summary);
            Ok(())
        })?;
        let count = entries.len();
        let context_file = staging
            .join("collection.json")
            .exists()
            .then_some("collection.json");
        write_json(
            &staging.join("manifest.json"),
            &json!({"format":"posttool-ai-bundle","version":1,"createdAt":chrono::Utc::now().to_rfc3339(),"count":count,"contextFile":context_file,"requests":entries}),
        )?;
        std::fs::write(staging.join("README.md"), "# postTool 请求资料\n\n请先读取 manifest.json，再按 requests 中的 directory 进入每个请求目录，读取 index.json。\n\n- request.json：复制时的编辑配置，包含地址、方法、Query 参数、Headers、鉴权、表单及网络设置；正文、GraphQL 变量、前后置脚本由 *File 字段引用独立文件。\n- resolved-request.json（如有）：按复制时所选环境解析的配置，没有执行脚本，也没有发送网络请求。\n- sent-request.json（如有）：与 response.json 对应的最近一次实际发送快照，已解析变量、鉴权并执行前置脚本；正文独立存储。编辑配置可能在发送后被修改，请以此文件分析响应。\n- response.json / response-body.*（如有）：响应状态、Headers、耗时、字节数与完整缓存正文。二进制文件保留原始字节；失败或取消的响应可能只有部分正文。\n- attachments/：binary/multipart 上传文件副本，字段和原路径映射见 index.json。\n- diagnostics.json：请求错误、断言结果和 Console 日志。\n\n缺失的响应、变量解析失败或附件读取失败均在 index.json / manifest.json 中明确标注。历史记录的原始配置可能已脱敏且不含正文。文件路径均相对此请求目录。\n").map_err(err)?;
        if context_file.is_some() {
            std::fs::OpenOptions::new().append(true).open(staging.join("README.md")).map_err(err)?
                .write_all("\n## 集合上下文\n\n请先读取 collection.json：它记录本次集合/文件夹及祖先的层级、变量、继承鉴权和前后置脚本文件。environment.json / globals.json 是复制时的环境与全局变量。未打开的已保存请求由数据库逐条读取；如没有 resolved-request.json，请结合这些上下文解析 request.json 中的变量与继承鉴权。复制不会运行脚本或发送网络请求。\n".as_bytes()).map_err(err)?;
        }
        std::fs::rename(&staging, &directory).map_err(err)?;
        let clipboard_text = format!("请查看这批 postTool 请求资料（共 {count} 条），根据我的后续问题分析。\n资料目录：{}\n请先读取：{}\n请求索引：{}\n每个请求子目录中的 index.json 说明请求配置、实际发送快照、独立 request/response 正文文件及附件的位置。请按需读取完整文件，并留意缺失数据或未完成响应的说明。", visible_path(&directory), visible_path(&directory.join("README.md")), visible_path(&directory.join("manifest.json")));
        Ok(CopyResult {
            directory: visible_path(&directory),
            count,
            clipboard_text,
            clipboard_error: None,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

#[tauri::command]
pub async fn posttool_copy_requests(items: Vec<CopyItem>) -> Result<CopyResult, String> {
    let executable = std::env::current_exe().map_err(err)?;
    let base = application_directory(&executable)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = create_bundle(&base, items, hub())?;
        result.clipboard_error = crate::clipboard::write_text(&result.clipboard_text).err();
        Ok(result)
    })
    .await
    .map_err(err)?
}

fn create_collection_bundle(
    base: &Path,
    store: &Path,
    id: &str,
    open_items: Vec<CopyItem>,
    context: Value,
    h: &Hub,
) -> Result<CopyResult, String> {
    if open_items.len() > 12 {
        return Err("打开的请求超过 12 个标签上限".into());
    }
    let mut opened: HashMap<String, CopyItem> = open_items
        .into_iter()
        .filter_map(|item| {
            item.request["id"]
                .as_str()
                .map(str::to_string)
                .map(|id| (id, item))
        })
        .collect();
    create_bundle_from_source(base, h, |staging, visitor| {
        write_json(&staging.join("environment.json"), &context["environment"])?;
        write_json(&staging.join("globals.json"), &context["globals"])?;
        let mut hierarchy = Vec::new();
        storage::visit_collection_documents(store, id, &mut |document| {
            if document.kind == "request" {
                let item = opened.remove(&document.id).unwrap_or(CopyItem {
                    request: document.payload,
                    source: "collection".into(),
                    response_id: None,
                    resolved: None,
                    resolution_error: None,
                    error: String::new(),
                    tests: vec![],
                    logs: vec![],
                });
                visitor(item)?;
            } else {
                validate_id(&document.id)?;
                let folder = format!("context/{}", document.id);
                std::fs::create_dir_all(staging.join(&folder)).map_err(err)?;
                write_request(&staging.join(&folder), "settings", document.payload)?;
                hierarchy.push(json!({"id":document.id,"parent":document.parent,"name":document.name,"kind":document.kind,"file":format!("{folder}/settings.json")}));
            }
            Ok(())
        })?;
        write_json(
            &staging.join("collection.json"),
            &json!({"id":id,"hierarchy":hierarchy,"environmentFile":"environment.json","globalsFile":"globals.json"}),
        )
    })
}

#[tauri::command]
pub async fn posttool_copy_collection(
    app: AppHandle,
    id: String,
    open_items: Vec<CopyItem>,
    context: Value,
) -> Result<CopyResult, String> {
    let base = application_directory(&std::env::current_exe().map_err(err)?)?;
    let store = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = create_collection_bundle(&base, &store, &id, open_items, context, hub())?;
        result.clipboard_error = crate::clipboard::write_text(&result.clipboard_text).err();
        Ok(result)
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::posttool::tests::{isolated_hub, spec};

    fn item(name: &str) -> CopyItem {
        serde_json::from_value(json!({"request":{
            "name":name,"method":"POST","url":"https://example.test/edited",
            "params":[{"key":"q","value":"一","enabled":true},{"key":"q","value":"二","enabled":true}],
            "headers":[{"key":"Authorization","value":"Bearer example-secret","enabled":true}],
            "auth":{"type":"bearer","token":"example-secret"},
            "bodyMode":"json","body":"{\"edited\":true}",
            "graphqlVariables":"{}","preScript":"console.log('before')","postScript":"console.log('after')"
        },"source":"editor"})).unwrap()
    }
    fn json_file(path: impl AsRef<Path>) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn app_directory_uses_executable_parent_and_macos_bundle_parent() {
        assert_eq!(
            application_directory(Path::new("/apps/AgentDock.exe")).unwrap(),
            PathBuf::from("/apps")
        );
        assert_eq!(
            application_directory(Path::new("/apps/Agent Dock.app/Contents/MacOS/agent-dock"))
                .unwrap(),
            PathBuf::from("/apps")
        );
    }

    #[cfg(windows)]
    #[test]
    fn portable_windows_paths_follow_renamed_exe_and_hide_extended_prefix() {
        assert_eq!(
            application_directory(Path::new(r"D:\aidock\AgentDock-0.1.20.exe")).unwrap(),
            PathBuf::from(r"D:\aidock")
        );
        assert_eq!(
            application_directory(Path::new(r"E:\我的工具\绿色包\自定义名称.exe")).unwrap(),
            PathBuf::from(r"E:\我的工具\绿色包")
        );
        assert_eq!(
            visible_path(Path::new(r"\\?\D:\aidock\tmp\copy")),
            r"D:\aidock\tmp\copy"
        );
        assert_eq!(
            visible_path(Path::new(r"\\?\UNC\server\共享\tmp")),
            r"\\server\共享\tmp"
        );
    }

    #[test]
    fn portable_copy_uses_executable_folder_after_relocation() {
        let temp = tempfile::tempdir().unwrap();
        let before = temp.path().join("旧目录").join("AgentDock-0.1.20.exe");
        let after = temp.path().join("工具箱").join("绿色 包").join("renamed.exe");
        let h = isolated_hub();
        for exe in [before, after] {
            let base = application_directory(&exe).unwrap();
            let result = create_bundle(&base, vec![item("test")], &h).unwrap();
            assert_eq!(
                Path::new(&result.directory).parent().unwrap(),
                base.join("tmp")
            );
            assert!(result
                .clipboard_text
                .contains(&visible_path(&base.join("tmp"))));
        }
    }

    #[test]
    fn collection_copy_includes_all_pages_descendants_and_ancestor_context() {
        let base = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let root = uuid::Uuid::new_v4().to_string();
        let child = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        let make = |id: String, kind: &str, parent: &str, name: &str, mut payload: Value| {
            payload["id"] = json!(id);
            payload["parent"] = json!(parent);
            storage::Document {
                id,
                kind: kind.into(),
                parent: parent.into(),
                name: name.into(),
                method: "GET".into(),
                url: String::new(),
                updated: 0,
                payload,
            }
        };
        let mut documents = vec![
            make(
                root.clone(),
                "collection",
                "",
                "root",
                json!({"name":"root","variables":[{"key":"host","value":"https://api.test","enabled":true}],"auth":{"type":"bearer","token":"inherited-token"},"preScript":"console.log('inherited')"}),
            ),
            make(
                child.clone(),
                "folder",
                &root,
                "child",
                json!({"name":"child","auth":{"type":"inherit"}}),
            ),
            make(
                other.clone(),
                "collection",
                "",
                "other",
                json!({"name":"other"}),
            ),
        ];
        for i in 0..130 {
            documents.push(make(
                uuid::Uuid::new_v4().to_string(),
                "request",
                if i < 25 { &root } else { &child },
                &format!("request-{i}"),
                item(&format!("request-{i}")).request,
            ));
        }
        documents.push(make(
            uuid::Uuid::new_v4().to_string(),
            "request",
            &other,
            "excluded",
            item("excluded").request,
        ));
        storage::import_documents(store.path(), documents).unwrap();
        let h = isolated_hub();
        let result = create_collection_bundle(
            base.path(),
            store.path(),
            &root,
            vec![],
            json!({"environment":{"name":"dev"},"globals":[]}),
            &h,
        )
        .unwrap();
        assert_eq!(result.count, 130);
        let manifest = json_file(Path::new(&result.directory).join("manifest.json"));
        assert_eq!(manifest["contextFile"], "collection.json");
        assert_eq!(manifest["requests"].as_array().unwrap().len(), 130);
        assert!(!manifest["requests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["name"] == "excluded"));
        let collection = json_file(Path::new(&result.directory).join("collection.json"));
        assert_eq!(collection["hierarchy"].as_array().unwrap().len(), 2);
        let settings =
            json_file(Path::new(&result.directory).join(format!("context/{root}/settings.json")));
        assert_eq!(settings["auth"]["token"], "inherited-token");
        assert!(settings["preScriptFile"].is_string());
        let sub =
            create_collection_bundle(base.path(), store.path(), &child, vec![], json!({}), &h)
                .unwrap();
        assert_eq!(sub.count, 105);
        let context = json_file(Path::new(&sub.directory).join("collection.json"));
        assert_eq!(context["hierarchy"].as_array().unwrap().len(), 2);
        let empty =
            create_collection_bundle(base.path(), store.path(), &other, vec![], json!({}), &h)
                .unwrap();
        assert_eq!(empty.count, 1);
    }

    #[test]
    fn empty_collection_copies_settings_and_invalid_collection_leaves_no_partial_folder() {
        let base = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        storage::import_documents(
            store.path(),
            vec![storage::Document {
                id: id.clone(),
                kind: "collection".into(),
                parent: String::new(),
                name: "empty".into(),
                method: String::new(),
                url: String::new(),
                updated: 0,
                payload: json!({"name":"empty","variables":[]}),
            }],
        )
        .unwrap();
        let h = isolated_hub();
        let result =
            create_collection_bundle(base.path(), store.path(), &id, vec![], json!({}), &h)
                .unwrap();
        assert_eq!(result.count, 0);
        assert!(Path::new(&result.directory)
            .join("collection.json")
            .is_file());
        assert!(create_collection_bundle(
            base.path(),
            store.path(),
            &uuid::Uuid::new_v4().to_string(),
            vec![],
            json!({}),
            &h
        )
        .is_err());
        assert_eq!(
            std::fs::read_dir(base.path().join("tmp")).unwrap().count(),
            1
        );
    }

    #[test]
    fn full_response_and_sent_body_are_separate_from_edited_configuration() {
        let base = tempfile::tempdir().unwrap();
        let h = isolated_hub();
        let mut sent = spec("https://example.test/sent?q=one&q=two".into());
        sent.body_mode = "json".into();
        sent.body = "{\"large\":\"".to_string() + &"x".repeat(512 * 1024) + "\"}";
        let bytes: Vec<u8> = (0..1024 * 1024).map(|n| (n % 256) as u8).collect();
        let cache = base.path().join("cached.body");
        std::fs::write(&cache, &bytes).unwrap();
        h.inner.lock().unwrap().responses.insert(
            sent.id.clone(),
            ResponseEntry {
                meta: Arc::new(Mutex::new(ResponseMeta {
                    state: "done".into(),
                    status: 200,
                    bytes: bytes.len() as u64,
                    content_type: "application/octet-stream".into(),
                    ..Default::default()
                })),
                request: sent.clone(),
                path: cache,
                started: Instant::now(),
            },
        );
        let mut input = item("中文请求");
        input.response_id = Some(sent.id.clone());
        input.resolved = Some(spec("https://example.test/resolved-now".into()));
        let result = create_bundle(base.path(), vec![input], &h).unwrap();
        let root = Path::new(&result.directory);
        assert_eq!(root.parent().unwrap(), base.path().join("tmp"));
        let manifest = json_file(root.join("manifest.json"));
        let dir = root.join(manifest["requests"][0]["directory"].as_str().unwrap());
        let request = json_file(dir.join("request.json"));
        assert_eq!(request["url"], "https://example.test/edited");
        assert_eq!(request["headers"][0]["value"], "Bearer example-secret");
        assert_eq!(request["params"].as_array().unwrap().len(), 2);
        assert_eq!(request["auth"]["token"], "example-secret");
        assert!(request.get("body").is_none());
        assert_eq!(
            std::fs::read_to_string(dir.join(request["bodyFile"].as_str().unwrap())).unwrap(),
            "{\"edited\":true}"
        );
        let snapshot = json_file(dir.join("sent-request.json"));
        assert_eq!(snapshot["url"], sent.url);
        assert_eq!(
            std::fs::read_to_string(dir.join(snapshot["bodyFile"].as_str().unwrap())).unwrap(),
            sent.body
        );
        assert_eq!(std::fs::read(dir.join("response-body.bin")).unwrap(), bytes);
        assert_eq!(
            json_file(dir.join("response.json"))["capturedBytes"],
            1024 * 1024
        );
        assert!(std::fs::metadata(root.join("manifest.json")).unwrap().len() < 10_000);
        assert!(result.clipboard_text.len() < 3000);
        assert!(result.clipboard_text.contains(&result.directory));
    }

    #[test]
    fn batch_names_stay_inside_unique_folders_and_missing_cache_is_explicit() {
        let base = tempfile::tempdir().unwrap();
        let h = isolated_hub();
        let mut missing = item("../../CON:<bad>\\name");
        missing.source = "history".into();
        missing.response_id = Some(uuid::Uuid::new_v4().to_string());
        missing.resolution_error = Some("未定义的变量：host".into());
        let result = create_bundle(
            base.path(),
            vec![missing, item("../../CON:<bad>\\name")],
            &h,
        )
        .unwrap();
        let root = Path::new(&result.directory);
        let manifest = json_file(root.join("manifest.json"));
        assert_eq!(manifest["count"], 2);
        let a = manifest["requests"][0]["directory"].as_str().unwrap();
        let b = manifest["requests"][1]["directory"].as_str().unwrap();
        assert_ne!(a, b);
        assert!(!a.contains(['/', '\\', ':', '.', '<', '>']));
        assert!(root.join(a).is_dir());
        assert_eq!(manifest["requests"][0]["response"]["available"], false);
        assert!(manifest["requests"][0]["response"]["reason"]
            .as_str()
            .unwrap()
            .contains("释放"));
        assert_eq!(
            manifest["requests"][0]["notes"].as_array().unwrap().len(),
            3
        );
        assert_eq!(
            std::fs::read_dir(base.path().join("tmp")).unwrap().count(),
            1
        );
        let second = create_bundle(base.path(), vec![item("same")], &h).unwrap();
        assert_ne!(result.directory, second.directory);
        assert!(root.exists());
    }

    #[test]
    fn multipart_files_are_copied_as_bytes_and_unavailable_files_are_recorded() {
        let base = tempfile::tempdir().unwrap();
        let upload = base.path().join("upload.bin");
        std::fs::write(&upload, [0, 255, 128, 10]).unwrap();
        let mut input = item("upload");
        input.request["bodyMode"] = json!("multipart");
        input.request["fields"] = json!([
            {"key":"../../file","value":upload,"file":true,"enabled":true},
            {"key":"missing","value":base.path().join("missing"),"file":true,"enabled":true},
            {"key":"disabled","value":upload,"file":true,"enabled":false}
        ]);
        let result = create_bundle(base.path(), vec![input], &isolated_hub()).unwrap();
        let root = Path::new(&result.directory);
        let manifest = json_file(root.join("manifest.json"));
        let record = &manifest["requests"][0];
        let attachments = record["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 2);
        let dir = root.join(record["directory"].as_str().unwrap());
        assert_eq!(
            std::fs::read(dir.join(attachments[0]["file"].as_str().unwrap())).unwrap(),
            [0, 255, 128, 10]
        );
        assert!(attachments[1]["file"].is_null());
        assert!(attachments[1]["error"].is_string());
    }

    #[test]
    fn running_request_fails_without_leaving_partial_bundle() {
        let base = tempfile::tempdir().unwrap();
        let h = isolated_hub();
        let mut input = item("running");
        let id = uuid::Uuid::new_v4().to_string();
        input.response_id = Some(id.clone());
        h.inner
            .lock()
            .unwrap()
            .active
            .insert(id, CancellationToken::new());
        assert!(create_bundle(base.path(), vec![item("first"), input], &h).is_err());
        assert_eq!(
            std::fs::read_dir(base.path().join("tmp")).unwrap().count(),
            0
        );
        assert!(create_bundle(base.path(), vec![], &h).is_err());
    }

    #[test]
    fn cancelled_zero_byte_response_keeps_metadata_and_empty_body() {
        let base = tempfile::tempdir().unwrap();
        let h = isolated_hub();
        let sent = spec("https://example.test/cancelled".into());
        let mut input = item("cancelled");
        input.response_id = Some(sent.id.clone());
        h.inner.lock().unwrap().responses.insert(
            sent.id.clone(),
            ResponseEntry {
                meta: Arc::new(Mutex::new(ResponseMeta {
                    state: "cancelled".into(),
                    error: Some("请求已取消".into()),
                    ..Default::default()
                })),
                request: sent,
                path: base.path().join("not-created.body"),
                started: Instant::now(),
            },
        );
        let result = create_bundle(base.path(), vec![input], &h).unwrap();
        let root = Path::new(&result.directory);
        let manifest = json_file(root.join("manifest.json"));
        let record = &manifest["requests"][0];
        let dir = root.join(record["directory"].as_str().unwrap());
        assert_eq!(record["response"]["state"], "cancelled");
        assert_eq!(
            std::fs::metadata(dir.join("response-body.bin"))
                .unwrap()
                .len(),
            0
        );
        assert_eq!(json_file(dir.join("response.json"))["error"], "请求已取消");
        assert!(!record["notes"].as_array().unwrap().is_empty());
    }
}
