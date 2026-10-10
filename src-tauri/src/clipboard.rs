use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardContents {
    pub text: String,
    pub has_image: bool,
}

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

pub fn read_contents() -> ClipboardContents {
    ClipboardContents {
        text: read_text().unwrap_or_default(),
        has_image: has_image(),
    }
}

const CF_BITMAP: u32 = 2;
const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;

pub fn bitmap_formats_present(available: impl Fn(u32) -> bool) -> bool {
    available(CF_BITMAP) || available(CF_DIB) || available(CF_DIBV5)
}

fn has_image() -> bool {
    #[cfg(windows)]
    {
        windows_has_image()
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_image().map(|_| ()))
            .is_ok()
    }
}

#[cfg(windows)]
fn windows_has_image() -> bool {
    use windows_sys::Win32::System::DataExchange::IsClipboardFormatAvailable;
    if bitmap_formats_present(|format| unsafe { IsClipboardFormatAvailable(format) != 0 }) {
        return true;
    }
    for name in [
        "PNG",
        "image/png",
        "JFIF",
        "GIF",
        "image/jpeg",
        "image/gif",
        "image/webp",
        "image/bmp",
    ] {
        let format = register_clipboard_format(name);
        if format != 0 && unsafe { IsClipboardFormatAvailable(format) != 0 } {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn register_clipboard_format(name: &str) -> u32 {
    use windows_sys::Win32::System::DataExchange::RegisterClipboardFormatW;
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    wide.push(0);
    unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_dib_and_dibv5_count_as_images() {
        assert!(bitmap_formats_present(|format| format == CF_DIB));
        assert!(bitmap_formats_present(|format| format == CF_BITMAP));
        assert!(bitmap_formats_present(|format| format == CF_DIBV5));
        assert!(!bitmap_formats_present(|_| false));
    }
}
