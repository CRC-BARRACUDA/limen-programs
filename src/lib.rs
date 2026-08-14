//! `programs` — a native Limen module that lists software installed on **this**
//! machine.
//!
//! Provides `programs.local`. Methods: `list` (raw data, for other modules),
//! plus `ui`/`scan` for the built-in view — the UI does **not** scan on open; it
//! enumerates only when the user presses **Scan**. Sources are per-OS:
//! - **Windows** — the `Uninstall` registry keys (HKLM 64- and 32-bit, plus
//!   **HKCU** for per-user installs) — i.e. what Add/Remove Programs shows.
//! - **Linux** — system packages (`dpkg`, `rpm`), universal packages
//!   (`flatpak`, `snap`), **XDG desktop entries** (system dirs *and*
//!   `~/.local/share/applications`), user-local executables in **`~/.local/bin`**,
//!   and `/opt` install trees.
//!
//! The results table is interactive: right-click a row for **About** / **Open
//! location**, or double-click for its details. When a report provider is loaded,
//! a **Make Report** action is offered.
//!
//! Every entry is emitted in one shared schema (see [`entry`]):
//!
//! `source`, `name`, `version`, `publisher`, `location`, `scope`.
//!
//! ```text
//!   handler    what a call does, and what is remembered between calls
//!   entry      the shared record, and what can be done with one
//!   view       everything the module draws
//!   report     handing the last scan to a report provider
//!   clipboard  putting a path where the user can paste it
//!   linux/     what is installed, as Linux records it
//!   windows/   what is installed, as Windows records it
//! ```
//!
//! Built as a native (`cdylib`) module using `limen-sdk-rust`.

mod clipboard;
mod entry;
mod handler;
mod report;
mod view;

#[cfg(test)]
mod tests;

// This was one file, and reads best as one: each part takes `use crate::*` and
// finds everything the way it did before the split, rather than every file
// carrying a list of its neighbours that has to be maintained by hand.
pub(crate) use clipboard::*;
pub(crate) use entry::*;
pub(crate) use handler::*;
pub(crate) use report::*;
pub(crate) use view::*;

pub(crate) use std::collections::HashMap;
pub(crate) use std::path::Path;

pub(crate) use limen_sdk_rust::ui::{
    button, label, menu_item, notice, row, select, separator, table, text, window, MenuItem, Widget,
};
pub(crate) use limen_sdk_rust::{json, rpc, Catalog, Handler, Host, RpcError, Value};

use limen_sdk_rust::export_module;

/// Rows shown per page — the machine can have thousands of programs, so the
/// table is paginated to keep each view light.
pub(crate) const PAGE_SIZE: usize = 100;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{list_programs, package_path};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub(crate) use windows::{list_programs, package_path};

/// Fallback for platforms without a collector.
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub(crate) fn list_programs() -> Value {
    json!({
        "os": std::env::consts::OS,
        "note": "installed-program listing is only implemented for Windows and Linux",
        "total": 0,
        "entries": [],
    })
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub(crate) fn package_path(_source: &str, _name: &str) -> Option<String> {
    None
}

/// This module's own translations (its `resources/locales/*.toml`, embedded).
/// English is the default/fallback; `host.locale()` selects the active one at
/// render time.
pub(crate) fn catalog() -> &'static Catalog {
    static C: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        Catalog::new(&[
            ("en", include_str!("../resources/locales/en.toml")),
            ("uk", include_str!("../resources/locales/uk.toml")),
        ])
    })
}

export_module!(Programs);
