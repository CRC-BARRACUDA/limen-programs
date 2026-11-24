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
//!
//! ```text
//!   packages  dpkg, rpm, flatpak, snap
//!   desktop   XDG .desktop entries, system and user
//!   local     ~/.local/bin and /opt
//! ```

mod desktop;
mod local;
mod packages;

use std::path::Path;
use std::process::Command;

use crate::*;

use desktop::collect_desktop;
use local::{collect_local_bin, collect_opt};
use packages::{collect_dpkg, collect_flatpak, collect_rpm, collect_snap};

pub fn list_programs() -> Value {
    let mut entries: Vec<Value> = Vec::new();
    collect_dpkg(&mut entries);
    collect_rpm(&mut entries);
    collect_flatpak(&mut entries);
    collect_snap(&mut entries);
    collect_desktop(&mut entries);
    collect_local_bin(&mut entries);
    collect_opt(&mut entries);

    entries.sort_by_key(|d| field(d, "name").to_lowercase());

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
pub(crate) fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
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
        .or_else(|| {
            files
                .lines()
                .find(|l| Path::new(l).is_file())
                .map(str::to_string)
        })
}

/// Whether `p` is an executable under a standard bin directory.
fn is_bin(p: &str) -> bool {
    const DIRS: [&str; 5] = ["/usr/bin/", "/bin/", "/usr/sbin/", "/sbin/", "/usr/local/bin/"];
    DIRS.iter().any(|d| p.starts_with(d)) && Path::new(p).is_file()
}
