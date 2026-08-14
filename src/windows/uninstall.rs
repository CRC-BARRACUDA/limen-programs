//! Reading one `Uninstall` view, and dropping what isn't a program.

use std::collections::HashSet;

use winreg::{RegKey, HKEY};

use super::field;
use crate::*;

fn sval(k: &RegKey, name: &str) -> String {
    k.get_value::<String, _>(name).unwrap_or_default().trim().to_string()
}

fn u32val(k: &RegKey, name: &str) -> u32 {
    k.get_value::<u32, _>(name).unwrap_or(0)
}

pub(super) fn collect(out: &mut Vec<Value>, hive: HKEY, path: &str, scope: &str) {
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
pub(super) fn dedup(entries: &mut Vec<Value>) {
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
