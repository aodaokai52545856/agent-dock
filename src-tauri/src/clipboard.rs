pub fn write_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .map_err(|err| format!("无法写入剪贴板：{err}"))
}

pub fn read_text() -> Result<String, String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .map_err(|err| format!("无法读取剪贴板：{err}"))
}
