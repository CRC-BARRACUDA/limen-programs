//! Linux installed-program enumeration. Fans out to several sources so both
//! system packages and **user-local** installs are covered:
//!
//! - **`dpkg`** / **`rpm`** — system package managers.
//! - **`flatpak`** / **`snap`** — universal packages (user or system).
//! - **XDG desktop entries** — `/usr/share/applications`,
//!   `/usr/local/share/applications`, and **`~/.local/share/applications`**
//!   (where user-installed apps and AppImages register themselves).
//! - **`~/.local/bin`** — user-installed executables (pip/pipx, cargo, manual).
//! - **`/opt`** — vendor install trees.
//!
//! Each source tags its entries with a `source` and a `scope` (`system`/`user`),
//! and the shared schema from [`crate::entry`].

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use limen_sdk_rust::{json, Value};

use crate::entry;

pub fn list_programs() -> Value {
    let mut entries: Vec<Value> = Vec::new();
    collect_dpkg(&mut entries);
    collect_rpm(&mut entries);
    collect_flatpak(&mut entries);
    collect_snap(&mut entries);
    collect_desktop(&mut entries);
    collect_local_bin(&mut entries);
    collect_opt(&mut entries);

    entries.sort_by(|a, b| field(a, "name").to_lowercase().cmp(&field(b, "name").to_lowercase()));

    json!({
        "os": "linux",
        "note": "Installed programs from system packages (dpkg/rpm), universal \
                 packages (flatpak/snap), XDG desktop entries (system and \
                 ~/.local/share/applications), user executables in ~/.local/bin, \
                 and /opt install trees.",
        "total": entries.len(),
        "entries": entries,
    })
}

fn field(d: &Value, key: &str) -> String {
    d.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Run a command, returning its stdout as a String on success (None if the
/// program is absent or exits non-zero — a source that isn't present is skipped).
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A representative path for a package (`dpkg`/`rpm`) — its main executable if it
/// has one, else the first file it owns. Resolved from the package's file list,
/// so it costs one subprocess and is called only when the user acts on a specific
/// package (never during a scan). `None` if the package isn't found.
pub(crate) fn package_path(source: &str, name: &str) -> Option<String> {
    let files = match source {
        "rpm" => run("rpm", &["-ql", name])?,
        "dpkg" => run("dpkg", &["-L", name])?,
        _ => return None,
    };
    let bins: Vec<&str> = files.lines().filter(|l| is_bin(l)).collect();
    // Prefer a binary named like the package, then any binary, then any file.
    bins.iter()
        .find(|l| l.rsplit('/').next() == Some(name))
        .or_else(|| bins.first())
        .map(|s| s.to_string())
        .or_else(|| files.lines().find(|l| Path::new(l).is_file()).map(str::to_string))
}

/// Whether `p` is an executable under a standard bin directory.
fn is_bin(p: &str) -> bool {
    const DIRS: [&str; 5] = ["/usr/bin/", "/bin/", "/usr/sbin/", "/sbin/", "/usr/local/bin/"];
    DIRS.iter().any(|d| p.starts_with(d)) && Path::new(p).is_file()
}

// --------------------------------------------------------------------------- //
// Package managers
// --------------------------------------------------------------------------- //
fn collect_dpkg(out: &mut Vec<Value>) {
    let fmt = "-f=${Package}\t${Version}\t${Maintainer}\t${db:Status-Status}\n";
    let Some(s) = run("dpkg-query", &["-W", fmt]) else {
        return;
    };
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 || f[3] != "installed" || f[0].is_empty() {
            continue;
        }
        out.push(entry("dpkg", f[0].into(), f[1].into(), f[2].into(), String::new(), "system"));
    }
}

fn collect_rpm(out: &mut Vec<Value>) {
    let Some(s) = run("rpm", &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{VENDOR}\n"]) else {
        return;
    };
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.is_empty() || f[0].is_empty() {
            continue;
        }
        let publisher = f.get(2).copied().filter(|v| *v != "(none)").unwrap_or("");
        out.push(entry(
            "rpm",
            f[0].into(),
            f.get(1).copied().unwrap_or("").into(),
            publisher.into(),
            String::new(),
            "system",
        ));
    }
}

fn collect_flatpak(out: &mut Vec<Value>) {
    let Some(s) = run("flatpak", &["list", "--app", "--columns=name,version,origin,installation"])
    else {
        return;
    };
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.is_empty() || f[0].is_empty() {
            continue;
        }
        let scope = if f.get(3) == Some(&"user") { "user" } else { "system" };
        out.push(entry(
            "flatpak",
            f[0].into(),
            f.get(1).copied().unwrap_or("").into(),
            f.get(2).copied().unwrap_or("").into(),
            String::new(),
            scope,
        ));
    }
}

fn collect_snap(out: &mut Vec<Value>) {
    let Some(s) = run("snap", &["list"]) else {
        return;
    };
    // Columns: Name  Version  Rev  Tracking  Publisher  Notes
    for line in s.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() || f[0].is_empty() {
            continue;
        }
        out.push(entry(
            "snap",
            f[0].into(),
            f.get(1).copied().unwrap_or("").into(),
            f.get(4).copied().unwrap_or("").into(),
            String::new(),
            "system",
        ));
    }
}

// --------------------------------------------------------------------------- //
// XDG desktop entries (system dirs + ~/.local/share/applications)
// --------------------------------------------------------------------------- //
fn collect_desktop(out: &mut Vec<Value>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let user_apps = format!("{home}/.local/share/applications");
    let dirs: [(&str, &str); 3] = [
        ("/usr/share/applications", "system"),
        ("/usr/local/share/applications", "system"),
        (user_apps.as_str(), "user"),
    ];
    for (dir, scope) in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(name) = parse_desktop(&path) {
                let loc = path.to_string_lossy().into_owned();
                out.push(entry("xdg-desktop", name, String::new(), String::new(), loc, scope));
            }
        }
    }
}

/// Read a `.desktop` file's application name. Returns None for non-application
/// entries (directories/links) and hidden ones (`NoDisplay=true`/`Hidden=true`).
fn parse_desktop(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut name: Option<String> = None;
    let mut is_app = true;
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| v.to_string());
        } else if let Some(v) = line.strip_prefix("Type=") {
            is_app = v == "Application";
        } else if let Some(v) = line.strip_prefix("NoDisplay=") {
            if v.eq_ignore_ascii_case("true") {
                return None;
            }
        } else if let Some(v) = line.strip_prefix("Hidden=") {
            if v.eq_ignore_ascii_case("true") {
                return None;
            }
        }
    }
    is_app.then_some(()).and(name)
}

// --------------------------------------------------------------------------- //
// User-local executables + /opt
// --------------------------------------------------------------------------- //
fn collect_local_bin(out: &mut Vec<Value>) {
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

fn collect_opt(out: &mut Vec<Value>) {
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

#[cfg(test)]
mod tests {
    #[test]
    fn envelope_shape_is_consistent() {
        let v = super::list_programs();
        assert_eq!(v.get("os").and_then(|o| o.as_str()), Some("linux"));
        let entries = v.get("entries").and_then(|e| e.as_array()).expect("entries[]");
        let total = v.get("total").and_then(|t| t.as_u64()).unwrap();
        assert_eq!(total, entries.len() as u64);
        for e in entries {
            for key in ["source", "name", "version", "publisher", "location", "scope"] {
                assert!(e.get(key).and_then(|x| x.as_str()).is_some(), "missing {key}");
            }
        }
    }
}
