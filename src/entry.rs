//! One installed program, in the shared schema every collector emits — and what
//! can be done with one.
//!
//! The fields are data, not prose: a source is called `dpkg` and a scope
//! `system` in every language, so nothing here is translated. Only the words
//! *about* an entry (its column headings, its menu items) are — see [`view`].

use crate::*;

/// Build one installed-program record in the shared schema. Used by every
/// platform collector.
///
/// - `source` — where it is registered (`registry-uninstall`, `dpkg`, `rpm`,
///   `flatpak`, `snap`, `xdg-desktop`, `local-bin`, `opt`, …).
/// - `version` — the installed version, if known (empty otherwise).
/// - `publisher` — vendor / maintainer / origin, if known.
/// - `location` — the install directory, entry file, or executable path.
/// - `scope` — `system` (all users / machine) or `user`.
pub fn entry(
    source: &str,
    name: String,
    version: String,
    publisher: String,
    location: String,
    scope: &str,
) -> Value {
    json!({
        "source": source,
        "name": name,
        "version": version,
        "publisher": publisher,
        "location": location,
        "scope": scope,
    })
}

/// A cell value (empty string if the field is missing).
pub(crate) fn cell(d: &Value, key: &str) -> String {
    d.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// The six visible columns for an entry row.
pub(crate) fn row_cells(d: &Value) -> Vec<String> {
    vec![
        cell(d, "name"),
        cell(d, "version"),
        cell(d, "publisher"),
        cell(d, "source"),
        cell(d, "scope"),
        cell(d, "location"),
    ]
}

/// How to open an entry, decided from its actual `location`:
/// `(label key, host.open target, value)`. `None` when there's nothing to open —
/// some programs record no install path, and package names aren't filesystem
/// paths, so those rows simply get no open action.
pub(crate) fn open_kind(d: &Value) -> Option<(&'static str, &'static str, String)> {
    let location = cell(d, "location");
    if location.is_empty() {
        return None;
    }
    let p = Path::new(&location);
    if p.is_dir() {
        Some(("menu.open_location", "path", location)) // an install folder → file manager
    } else if p.is_file() {
        Some(("menu.open_file", "path", location)) // an executable / .desktop entry
    } else {
        None // a path that doesn't exist, or a bare package name
    }
}

/// Whether the entry is a system package (`dpkg`/`rpm`) — these record no
/// install directory, but a representative path can be resolved on demand from
/// the package's file list.
pub(crate) fn is_package(d: &Value) -> bool {
    matches!(cell(d, "source").as_str(), "rpm" | "dpkg")
}

/// Whether the entry has a path we can reveal / copy — either a recorded
/// `location`, or a package whose files we can query.
pub(crate) fn has_path(d: &Value) -> bool {
    !cell(d, "location").is_empty() || is_package(d)
}

/// The path to act on: the recorded `location`, or — for a package — a
/// representative file from its contents (resolved lazily, so it costs one
/// subprocess only when the user actually clicks). `None` if nothing resolves.
pub(crate) fn resolved_path(d: &Value) -> Option<String> {
    let loc = cell(d, "location");
    if !loc.is_empty() {
        return Some(loc);
    }
    package_path(&cell(d, "source"), &cell(d, "name"))
}
