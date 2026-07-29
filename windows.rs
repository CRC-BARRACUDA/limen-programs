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

use std::collections::HashSet;

use limen_sdk_rust::{json, Value};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::{RegKey, HKEY};

use crate::entry;

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_WOW: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

pub fn list_programs() -> Value {
    let mut entries: Vec<Value> = Vec::new();
    // HKLM 64-bit + 32-bit (WOW6432Node), then HKCU per-user installs.
    collect(&mut entries, HKEY_LOCAL_MACHINE, UNINSTALL, "system");
    collect(&mut entries, HKEY_LOCAL_MACHINE, UNINSTALL_WOW, "system");
    collect(&mut entries, HKEY_CURRENT_USER, UNINSTALL, "user");

    dedup(&mut entries);
    entries.sort_by(|a, b| field(a, "name").to_lowercase().cmp(&field(b, "name").to_lowercase()));

    json!({
        "os": "windows",
        "note": "Installed programs from the Uninstall registry keys (Add/Remove \
                 Programs): HKLM 64- and 32-bit, and HKCU per-user installs.",
        "total": entries.len(),
        "entries": entries,
    })
}

fn field(d: &Value, key: &str) -> String {
    d.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Windows records `InstallLocation` directly, so there is no lazy package
/// resolution — reveal/copy use that location when present.
pub(crate) fn package_path(_source: &str, _name: &str) -> Option<String> {
    None
}

fn sval(k: &RegKey, name: &str) -> String {
    k.get_value::<String, _>(name).unwrap_or_default().trim().to_string()
}

fn u32val(k: &RegKey, name: &str) -> u32 {
    k.get_value::<u32, _>(name).unwrap_or(0)
}

fn collect(out: &mut Vec<Value>, hive: HKEY, path: &str, scope: &str) {
    let root = RegKey::predef(hive);
    let Ok(uninstall) = root.open_subkey(path) else {
        return; // view missing (e.g. no WOW6432Node on 32-bit Windows) or no access
    };
    for name in uninstall.enum_keys().flatten() {
        let Ok(sub) = uninstall.open_subkey(&name) else {
            continue;
        };
        let display = sval(&sub, "DisplayName");
        if display.is_empty() {
            continue; // not shown in Add/Remove Programs
        }
        // Skip OS components and updates/patches — they aren't "programs".
        if u32val(&sub, "SystemComponent") == 1 {
            continue;
        }
        if matches!(sval(&sub, "ReleaseType").as_str(), "Update" | "Hotfix" | "Security Update") {
            continue;
        }
        if !sval(&sub, "ParentKeyName").is_empty() {
            continue; // a child entry of another product (e.g. a bundled update)
        }
        out.push(entry(
            "registry-uninstall",
            display,
            sval(&sub, "DisplayVersion"),
            sval(&sub, "Publisher"),
            sval(&sub, "InstallLocation"),
            scope,
        ));
    }
}

/// Drop duplicate (name, version, scope) rows — an all-users product can surface
/// in both the 64- and 32-bit views.
fn dedup(entries: &mut Vec<Value>) {
    let mut seen = HashSet::new();
    entries.retain(|d| {
        seen.insert(format!(
            "{}\u{1}{}\u{1}{}",
            field(d, "name"),
            field(d, "version"),
            field(d, "scope"),
        ))
    });
}
