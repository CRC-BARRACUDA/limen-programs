//! What is installed outside any package manager: user executables in
//! `~/.local/bin`, and vendor trees under `/opt`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::*;

pub(super) fn collect_local_bin(out: &mut Vec<Value>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let bin = format!("{home}/.local/bin");
    let Ok(rd) = std::fs::read_dir(&bin) else {
        return;
    };
    for e in rd.flatten() {
        let path = e.path();
        if !is_executable(&path) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let loc = path.to_string_lossy().into_owned();
        out.push(entry("local-bin", name, String::new(), String::new(), loc, "user"));
    }
}

pub(super) fn collect_opt(out: &mut Vec<Value>) {
    let Ok(rd) = std::fs::read_dir("/opt") else {
        return;
    };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let loc = path.to_string_lossy().into_owned();
        out.push(entry("opt", name, String::new(), String::new(), loc, "system"));
    }
}

/// Whether `path` (following symlinks) is a regular file with any execute bit —
/// pip/pipx/cargo drop symlinks into ~/.local/bin.
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
