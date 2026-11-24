//! XDG desktop entries — the system directories *and*
//! `~/.local/share/applications`, where user-installed apps and AppImages
//! register themselves.

use std::path::Path;

use crate::*;

pub(super) fn collect_desktop(out: &mut Vec<Value>) {
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
