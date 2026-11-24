use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

/// Each agy process writes its own log, including the conversation it actually streams.
pub(crate) struct ConversationLog {
    file: Option<File>,
    directory: TempDir,
    partial: Vec<u8>,
}

impl ConversationLog {
    pub(crate) fn new() -> io::Result<Self> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("agent-dock-agy-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o700));
        }
        Ok(Self {
            directory: builder.tempdir()?,
            file: None,
            partial: Vec::new(),
        })
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.directory.path().join("cli.log")
    }

    pub(crate) fn poll(&mut self) -> Option<String> {
        if self.file.is_none() {
            self.file = File::open(self.path()).ok();
        }
        let file = self.file.as_mut()?;
        let mut latest = None;
        let mut buffer = [0; 8192];
        // Bound the work per monitor tick, even if a tool writes a very long line.
        for _ in 0..32 {
            let count = file.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            for byte in &buffer[..count] {
                if *byte == b'\n' {
                    if let Some(id) = streamed_conversation(&self.partial) {
                        latest = Some(id);
                    }
                    self.partial.clear();
                } else if self.partial.len() < 512 {
                    self.partial.push(*byte);
                }
            }
        }
        latest
    }
}

fn streamed_conversation(line: &[u8]) -> Option<String> {
    // Match the CLI's foreground stream record, never IDs in prompts, search results or RPC payloads.
    let text = std::str::from_utf8(line).ok()?.trim_end_matches('\r');
    let (header, id) = text.split_once("] Streaming conversation ")?;
    let mut parts = header.split_whitespace();
    let date = parts.next()?.strip_prefix('I')?;
    if date.len() != 4 || !date.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    parts.next()?.parse::<chrono::NaiveTime>().ok()?;
    parts.next()?.parse::<u64>().ok()?;
    parts
        .next()?
        .strip_prefix("conversation_manager.go:")?
        .parse::<u32>()
        .ok()?;
    if parts.next().is_some() || id.len() != 36 {
        return None;
    }
    Some(Uuid::parse_str(id).ok()?.hyphenated().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn simultaneous_terminals_follow_their_own_foreground_streams() {
        let mut first = ConversationLog::new().unwrap();
        let mut second = ConversationLog::new().unwrap();
        assert_eq!(first.poll(), None);
        let mut a = File::create(first.path()).unwrap();
        let mut b = File::create(second.path()).unwrap();
        let id_a = Uuid::new_v4().to_string();
        let id_b = Uuid::new_v4().to_string();
        let prefix =
            "I1124 18:56:27.371444    1219 conversation_manager.go:1082] Streaming conversation ";
        write!(a, "{prefix}{id_a}").unwrap();
        writeln!(b, "{prefix}{id_b}").unwrap();
        assert_eq!(first.poll(), None);
        assert_eq!(second.poll().as_deref(), Some(id_b.as_str()));
        writeln!(a).unwrap();
        writeln!(a, "I1124 18:56:28.123456   12 server.go:1283] Starting conversation update stream for {id_b}").unwrap();
        writeln!(a, "quoted output: {prefix}{id_b}").unwrap();
        assert_eq!(first.poll().as_deref(), Some(id_a.as_str()));
        assert_eq!(second.poll(), None);
        writeln!(a, "{prefix}{id_b}\r").unwrap();
        assert_eq!(first.poll().as_deref(), Some(id_b.as_str()));
    }
}
