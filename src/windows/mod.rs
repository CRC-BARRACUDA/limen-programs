//! Windows installed-program enumeration — the `Uninstall` registry keys that
//! back Add/Remove Programs:
//!
//! - **HKLM** `…\CurrentVersion\Uninstall` — 64-bit machine-wide installs.
//! - **HKLM** `…\WOW6432Node\…\Uninstall` — 32-bit machine-wide installs.
//! - **HKCU** `…\CurrentVersion\Uninstall` — **per-user** installs.
//!
//! Entries without a `DisplayName`, OS components (`SystemComponent = 1`), and
//! updates/patches (`ReleaseType`/`ParentKeyName`) are skipped, so the list
//! matches what a user sees in Add/Remove Programs.
//!
//! ```text
//!   uninstall  reading one Uninstall view, and dropping what isn't a program
//! ```

mod uninstall;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::*;

use uninstall::{collect, dedup};

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_WOW: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

pub fn list_programs() -> Value {
    let mut entries: Vec<Value> = Vec::new();
    // HKLM 64-bit + 32-bit (WOW6432Node), then HKCU per-user installs.
    collect(&mut entries, HKEY_LOCAL_MACHINE, UNINSTALL, "system");
    collect(&mut entries, HKEY_LOCAL_MACHINE, UNINSTALL_WOW, "system");
    collect(&mut entries, HKEY_CURRENT_USER, UNINSTALL, "user");

    dedup(&mut entries);
    entries.sort_by_key(|d| field(d, "name").to_lowercase());

    json!({
        "os": "windows",
        "note": "Installed programs from the Uninstall registry keys (Add/Remove \
                 Programs): HKLM 64- and 32-bit, and HKCU per-user installs.",
        "total": entries.len(),
        "entries": entries,
    })
}

pub(crate) fn field(d: &Value, key: &str) -> String {
    d.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// An `InstallLocation` as a path somebody can open or paste.
///
/// Installers write the value themselves and some write it the way they would
/// on a command line, in quotes — the Visual Studio Installer does — which is
/// not a path any more: Explorer will not open it, and pasted it carries the
/// quotes along.
pub(crate) fn location(raw: &str) -> String {
    let raw = raw.trim();
    let inside = raw.strip_prefix('"').map(|rest| rest.strip_suffix('"').unwrap_or(rest));
    inside.unwrap_or(raw).trim().to_string()
}

/// Windows records `InstallLocation` directly, so there is no lazy package
/// resolution — reveal/copy use that location when present.
pub(crate) fn package_path(_source: &str, _name: &str) -> Option<String> {
    None
}
