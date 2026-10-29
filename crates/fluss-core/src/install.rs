//! Facts about how Fluss is installed, shared by the daemon, the CLI and the
//! installer helpers (`fluss setup …`, run by the NSIS installer).

/// Per-user uninstall entry written by the installer (`HKCU\…\Uninstall\Fluss`).
pub const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Fluss";
/// Value name under `HKCU\…\Run` (the installer removes the same name on uninstall).
pub const RUN_VALUE: &str = "Fluss";

/// How the running binaries got here.
pub fn kind_of(exe_dir: &std::path::Path, registered_dir: Option<&std::path::Path>) -> &'static str {
    let norm = |p: &std::path::Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    let dir = norm(exe_dir);
    if registered_dir.is_some_and(|r| norm(r) == dir) {
        "installed"
    } else if dir.ends_with("target\\debug") || dir.ends_with("target\\release") || dir.ends_with("target/debug") || dir.ends_with("target/release") {
        "dev"
    } else {
        "portable"
    }
}

fn same_dir(a: &str, b: &str) -> bool {
    a.trim().trim_matches('"').trim_end_matches(['\\', '/']).eq_ignore_ascii_case(b.trim_end_matches(['\\', '/']))
}

/// `PATH` with `dir` appended, or `None` if it is already there.
pub fn path_with(path: &str, dir: &str) -> Option<String> {
    if path.split(';').any(|p| same_dir(p, dir)) {
        return None;
    }
    let base = path.trim_end_matches(';');
    Some(if base.is_empty() { dir.to_string() } else { format!("{base};{dir}") })
}

/// `PATH` without `dir`, or `None` if it wasn't there. Other entries are kept as they were.
pub fn path_without(path: &str, dir: &str) -> Option<String> {
    let parts: Vec<&str> = path.split(';').collect();
    let kept: Vec<&str> = parts.iter().copied().filter(|p| !same_dir(p, dir)).collect();
    (kept.len() != parts.len()).then(|| kept.join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn path_edits() {
        let d = r"C:\Users\O'Brien\AppData\Local\Fluss";
        assert_eq!(path_with("", d).unwrap(), d);
        assert_eq!(path_with(r"%USERPROFILE%\bin;", d).unwrap(), format!(r"%USERPROFILE%\bin;{d}"));
        assert_eq!(path_with(&format!(r"C:\a;{}\", d.to_uppercase()), d), None);
        assert_eq!(path_without(&format!(r"C:\a;{d};%X%\b"), d).unwrap(), r"C:\a;%X%\b");
        assert_eq!(path_without(&format!(r#""{d}""#), d).unwrap(), "");
        assert_eq!(path_without(r"C:\a;C:\b", d), None);
    }

    #[test]
    fn kinds() {
        let inst = Path::new(r"C:\Users\x\AppData\Local\Fluss");
        assert_eq!(kind_of(inst, Some(Path::new(r"c:\users\x\appdata\local\fluss\"))), "installed");
        assert_eq!(kind_of(Path::new(r"C:\src\fluss\target\release"), Some(inst)), "dev");
        assert_eq!(kind_of(Path::new(r"D:\Tools\Fluss"), None), "portable");
    }
}
