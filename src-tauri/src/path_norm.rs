/// Normalize a filesystem path for equality checks on Windows and macOS.
pub fn normalize_path(input: &str) -> String {
    let mut s = input
        .trim()
        .trim_matches('"')
        .replace('\\', "/")
        .to_lowercase();
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    while s.len() > 3 && s.ends_with('/') {
        s.pop();
    }
    s
}

pub fn paths_equal(a: &str, b: &str) -> bool {
    normalize_path(a) == normalize_path(b)
}

pub fn folder_name(path: &str) -> String {
    let norm = normalize_path(path);
    norm.rsplit('/').find(|part| !part.is_empty()).unwrap_or(&norm).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_drive_slash_and_case() {
        assert!(paths_equal(
            r"D:\idea_jidian_projects\aitools\tools_portal",
            "D:/idea_jidian_projects/aitools/tools_portal"
        ));
        assert!(paths_equal(
            "d:/idea_jidian_projects/fin-mr-etl-forpk",
            r"D:\idea_jidian_projects\fin-mr-etl-forpk\"
        ));
    }

    #[test]
    fn trailing_slash_ignored() {
        assert!(paths_equal("C:/Users/PS", "C:/Users/PS/"));
    }

    #[test]
    fn different_paths_not_equal() {
        assert!(!paths_equal(r"D:\cliproxy", r"D:\aiquery"));
    }

    #[test]
    fn folder_name_from_windows_path() {
        assert_eq!(folder_name(r"D:\idea_jidian_projects\aitools"), "aitools");
    }

    #[test]
    fn unix_paths_equal_and_folder_name() {
        assert!(paths_equal("/Users/PS/proj", "/Users/PS/proj/"));
        assert!(paths_equal("/Users/PS/proj", "/users/ps/proj"));
        assert_eq!(folder_name("/Users/PS/idea_jidian_projects/aitools"), "aitools");
    }
}
