use super::*;
use crate::secret_box;
use rusqlite::{params, Connection};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
struct KeyFile {
    protect: String,
    wrapped: Option<String>,
}
fn connect(dir: &Path) -> Result<Connection, String> {
    std::fs::create_dir_all(dir).map_err(err)?;
    let db = Connection::open(dir.join("posttool.db")).map_err(err)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(err)?;
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS documents(id TEXT PRIMARY KEY,kind TEXT NOT NULL,parent TEXT NOT NULL,name TEXT NOT NULL,method TEXT NOT NULL,url TEXT NOT NULL,updated INTEGER NOT NULL,payload BLOB NOT NULL,nonce BLOB NOT NULL); CREATE INDEX IF NOT EXISTS document_kind_updated ON documents(kind,updated DESC); CREATE INDEX IF NOT EXISTS document_parent ON documents(parent);").map_err(err)?;
    Ok(db)
}
// Serialize key creation with all writes, avoiding two concurrent first-use keys.
static STORE_LOCK: Mutex<()> = Mutex::new(());
fn key(dir: &Path) -> Result<[u8; 32], String> {
    let path = dir.join("key.json");
    let raw_path = dir.join("key.bin");
    if path.exists() {
        let file: KeyFile =
            serde_json::from_slice(&std::fs::read(path).map_err(err)?).map_err(err)?;
        let wrapped = file.wrapped.map(|s| secret_box::decode(&s)).transpose()?;
        return secret_box::unwrap_master(&file.protect, wrapped.as_deref(), &raw_path);
    }
    #[cfg(windows)]
    let master = secret_box::random_master()?;
    #[cfg(not(windows))]
    let master = secret_box::ensure_master_file(&raw_path)?;
    let (protect, wrapped) = secret_box::wrap_master(&master)?;
    let file = KeyFile {
        protect,
        wrapped: wrapped.map(|b| secret_box::encode(&b)),
    };
    std::fs::write(path, serde_json::to_vec(&file).map_err(err)?).map_err(err)?;
    Ok(master)
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub kind: String,
    pub parent: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub updated: i64,
    #[serde(default)]
    pub payload: Value,
}
#[derive(Serialize)]
pub struct Page {
    items: Vec<Document>,
    total: i64,
}

// One decrypted document at a time: collection handoff does not load every body into IPC/RAM.
pub(super) fn visit_collection_documents(
    dir: &Path,
    id: &str,
    visitor: &mut dyn FnMut(Document) -> Result<(), String>,
) -> Result<(), String> {
    validate_id(id)?;
    let db = connect(dir)?;
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM documents WHERE id=?1 AND kind IN ('collection','folder'))",
        [id], |r| r.get(0),
    ).map_err(err)?;
    if !exists {
        return Err("集合或文件夹不存在，请刷新后重试".into());
    }
    let master = {
        let _lock = STORE_LOCK.lock().unwrap();
        key(dir)?
    };
    let mut stmt = db.prepare(
        "WITH RECURSIVE subtree(id) AS (
            SELECT id FROM documents WHERE id=?1
            UNION SELECT d.id FROM documents d JOIN subtree t ON d.parent=t.id WHERE d.kind IN ('folder','request')
        ), ancestors(id) AS (
            SELECT parent FROM documents WHERE id=?1
            UNION SELECT d.parent FROM documents d JOIN ancestors a ON d.id=a.id WHERE d.parent<>''
        ) SELECT id,kind,parent,name,method,url,updated,payload,nonce FROM documents
          WHERE id IN (SELECT id FROM subtree) OR id IN (SELECT id FROM ancestors)
          ORDER BY CASE kind WHEN 'collection' THEN 0 WHEN 'folder' THEN 1 ELSE 2 END,name,id"
    ).map_err(err)?;
    let mut rows = stmt.query([id]).map_err(err)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(err)? {
        count += 1;
        if count > 20_000 {
            return Err("集合超过 20,000 项，请按子文件夹分别复制".into());
        }
        let encrypted: Vec<u8> = row.get(7).map_err(err)?;
        let nonce: Vec<u8> = row.get(8).map_err(err)?;
        let payload =
            serde_json::from_slice(&secret_box::open(&master, &nonce, &encrypted)?).map_err(err)?;
        visitor(Document {
            id: row.get(0).map_err(err)?,
            kind: row.get(1).map_err(err)?,
            parent: row.get(2).map_err(err)?,
            name: row.get(3).map_err(err)?,
            method: row.get(4).map_err(err)?,
            url: row.get(5).map_err(err)?,
            updated: row.get(6).map_err(err)?,
            payload,
        })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn posttool_document_list(
    app: AppHandle,
    kind: String,
    parent: Option<String>,
    search: String,
    page: u32,
) -> Result<Page, String> {
    let dir = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let db=connect(&dir)?;
        let filter = format!("%{}%", search.replace('\\',"\\\\").replace('%',"\\%").replace('_',"\\_"));
        let where_sql="WHERE kind=?1 AND (?2 IS NULL OR parent=?2) AND (name LIKE ?3 ESCAPE '\\' OR url LIKE ?3 ESCAPE '\\')";
        let total=db.query_row(&format!("SELECT count(*) FROM documents {where_sql}"), params![kind,parent,filter], |r|r.get(0)).map_err(err)?;
        let mut stmt=db.prepare(&format!("SELECT id,kind,parent,name,method,url,updated FROM documents {where_sql} ORDER BY updated DESC,id LIMIT 100 OFFSET ?4")).map_err(err)?;
        let rows=stmt.query_map(params![kind,parent,filter,(page as i64).min(100_000)*100], |r| Ok(Document {id:r.get(0)?,kind:r.get(1)?,parent:r.get(2)?,name:r.get(3)?,method:r.get(4)?,url:r.get(5)?,updated:r.get(6)?,payload:Value::Null})).map_err(err)?;
        Ok(Page{items:rows.collect::<Result<Vec<_>,_>>().map_err(err)?,total})
    }).await.map_err(err)?
}
#[tauri::command]
pub async fn posttool_document_get(app: AppHandle, id: String) -> Result<Value, String> {
    let dir = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORE_LOCK.lock().unwrap();
        let db = connect(&dir)?;
        let (payload, nonce): (Vec<u8>, Vec<u8>) = db
            .query_row(
                "SELECT payload,nonce FROM documents WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        serde_json::from_slice(&secret_box::open(&key(&dir)?, &nonce, &payload)?).map_err(err)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn posttool_document_put(app: AppHandle, document: Document) -> Result<(), String> {
    validate_id(&document.id)?;
    if !matches!(
        document.kind.as_str(),
        "request" | "collection" | "folder" | "environment" | "globals" | "history" | "draft"
    ) {
        return Err("文档类型无效".into());
    }
    let bytes = serde_json::to_vec(&document.payload).map_err(err)?;
    if bytes.len() > 2 * 1024 * 1024 || document.name.len() > 1024 || document.url.len() > 8192 {
        return Err("文档超过大小上限".into());
    }
    let dir = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock=STORE_LOCK.lock().unwrap(); let mut db=connect(&dir)?;
        let (nonce,payload)=secret_box::seal(&key(&dir)?,&bytes)?;
        let tx=db.transaction().map_err(err)?;
        tx.execute("INSERT INTO documents(id,kind,parent,name,method,url,updated,payload,nonce) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET parent=excluded.parent,name=excluded.name,method=excluded.method,url=excluded.url,updated=excluded.updated,payload=excluded.payload,nonce=excluded.nonce",params![document.id,document.kind,document.parent,document.name,document.method,document.url,chrono::Utc::now().timestamp_millis(),payload,nonce]).map_err(err)?;
        tx.execute("DELETE FROM documents WHERE kind='history' AND (updated < ?1 OR id NOT IN (SELECT id FROM documents WHERE kind='history' ORDER BY updated DESC LIMIT 1000))",[chrono::Utc::now().timestamp_millis()-30*86400*1000]).map_err(err)?;
        tx.commit().map_err(err)
    }).await.map_err(err)?
}

pub(super) fn import_documents(dir: &Path, documents: Vec<Document>) -> Result<(), String> {
    if documents.len() > 10_000 {
        return Err("单次导入最多 10,000 项".into());
    }
    let _lock = STORE_LOCK.lock().unwrap();
    let mut db = connect(dir)?;
    let master = key(dir)?;
    let tx = db.transaction().map_err(err)?;
    let mut total = 0;
    for d in documents {
        validate_id(&d.id)?;
        if !matches!(
            d.kind.as_str(),
            "request" | "collection" | "folder" | "environment" | "globals"
        ) {
            return Err("导入文档类型无效".into());
        }
        let plain = serde_json::to_vec(&d.payload).map_err(err)?;
        total += plain.len();
        if plain.len() > 2 * 1024 * 1024
            || total > 16 * 1024 * 1024
            || d.name.len() > 1024
            || d.url.len() > 8192
        {
            return Err("导入文档超过大小上限".into());
        }
        let (nonce, payload) = secret_box::seal(&master, &plain)?;
        tx.execute("INSERT INTO documents(id,kind,parent,name,method,url,updated,payload,nonce) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![d.id,d.kind,d.parent,d.name,d.method,d.url,chrono::Utc::now().timestamp_millis(),payload,nonce]).map_err(err)?;
    }
    tx.commit().map_err(err)
}
#[tauri::command]
pub async fn posttool_document_import(
    app: AppHandle,
    documents: Vec<Document>,
) -> Result<(), String> {
    let dir = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || import_documents(&dir, documents))
        .await
        .map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn yaml_import_rejects_alias_expansion_before_parsing() {
        let json = parse_definition("openapi: 3.1.0\ninfo:\n  title: Demo\npaths: {}\n").unwrap();
        assert!(json.contains("Demo"));
        assert!(parse_definition("a: &huge [1,2,3]\nb: *huge\n").is_err());
        assert!(parse_definition("example: '*literal'\n").is_ok());
    }
    #[test]
    fn import_is_atomic_and_payload_is_encrypted() {
        let dir = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let d = Document {
            id: id.clone(),
            kind: "request".into(),
            parent: "".into(),
            name: "request".into(),
            method: "GET".into(),
            url: "".into(),
            updated: 0,
            payload: serde_json::json!({"token":"secret-unique-value"}),
        };
        assert!(import_documents(dir.path(), vec![d.clone(), d.clone()]).is_err());
        let db = connect(dir.path()).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM documents", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        import_documents(dir.path(), vec![d]).unwrap();
        let (payload, nonce): (Vec<u8>, Vec<u8>) = db
            .query_row(
                "SELECT payload,nonce FROM documents WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(!String::from_utf8_lossy(&payload).contains("secret-unique-value"));
        let master = key(dir.path()).unwrap();
        let plain = secret_box::open(&master, &nonce, &payload).unwrap();
        assert!(String::from_utf8(plain)
            .unwrap()
            .contains("secret-unique-value"));
    }
}
#[tauri::command]
pub async fn posttool_document_delete(app: AppHandle, id: String) -> Result<(), String> {
    let dir = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock=STORE_LOCK.lock().unwrap(); let db=connect(&dir)?;
        // A recursive CTE deletes descendants atomically, without leaving orphaned requests.
        db.execute("WITH RECURSIVE descendants(id) AS (SELECT id FROM documents WHERE id=?1 UNION SELECT d.id FROM documents d JOIN descendants p ON d.parent=p.id) DELETE FROM documents WHERE id IN (SELECT id FROM descendants)",[id]).map_err(err)?; Ok(())
    }).await.map_err(err)?
}
#[tauri::command]
pub async fn posttool_history_clear(app: AppHandle) -> Result<(), String> {
    let dir = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORE_LOCK.lock().unwrap();
        connect(&dir)?
            .execute("DELETE FROM documents WHERE kind='history'", [])
            .map_err(err)?;
        Ok(())
    })
    .await
    .map_err(err)?
}
#[tauri::command]
pub async fn posttool_file_read(path: String) -> Result<String, String> {
    String::from_utf8(read_limited(&path, 16 * 1024 * 1024).await?)
        .map_err(|_| "文件必须为 UTF-8 文本".into())
}

fn parse_definition(text: &str) -> Result<String, String> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("定义文件超过 16 MiB".into());
    }
    // Refuse YAML anchors/aliases before constructing an expanded tree.
    for line in text.lines() {
        let mut quote = '\0';
        let mut escaped = false;
        let mut previous = ' ';
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if escaped {
                escaped = false;
                continue;
            }
            if quote == '"' && c == '\\' {
                escaped = true;
                continue;
            }
            if quote != '\0' {
                if c == quote {
                    quote = '\0'
                }
                continue;
            }
            if c == '\'' || c == '"' {
                quote = c;
                continue;
            }
            if c == '#' && previous.is_whitespace() {
                break;
            }
            if matches!(c, '*' | '&')
                && (previous.is_whitespace() || matches!(previous, ':' | '[' | '{' | ','))
                && chars
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                return Err("YAML 锚点/别名暂不支持，请展开引用或转换为 JSON 后导入".into());
            }
            previous = c;
        }
    }
    let value: serde_json::Value = serde_yaml::from_str(text).map_err(err)?;
    let result = serde_json::to_string(&value).map_err(err)?;
    if result.len() > 16 * 1024 * 1024 {
        return Err("定义文件展开后超过 16 MiB".into());
    }
    Ok(result)
}
#[tauri::command]
pub async fn posttool_parse_definition(text: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || parse_definition(&text))
        .await
        .map_err(err)?
}
#[tauri::command]
pub async fn posttool_file_write(path: String, text: String) -> Result<(), String> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("导出超过 16 MiB 上限".into());
    }
    tokio::fs::write(path, text).await.map_err(err)
}
