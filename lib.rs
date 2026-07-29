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
//! Built as a native (`cdylib`) module using `limen-sdk-rust`.

use std::collections::HashMap;
use std::path::Path;

use limen_sdk_rust::ui::{
    button, label, menu_item, row, select, separator, table, text, window, MenuItem, Widget,
};
use limen_sdk_rust::{export_module, json, rpc, Handler, Host, RpcError, Value};

/// Rows shown per page — the machine can have thousands of programs, so the
/// table is paginated to keep each view light.
const PAGE_SIZE: usize = 100;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{list_programs, package_path};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::{list_programs, package_path};

/// Fallback for platforms without a collector.
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn list_programs() -> Value {
    json!({
        "os": std::env::consts::OS,
        "note": "installed-program listing is only implemented for Windows and Linux",
        "total": 0,
        "entries": [],
    })
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn package_path(_source: &str, _name: &str) -> Option<String> {
    None
}

#[derive(Default)]
struct Programs {
    /// Whether the user has scanned this session. Once true, reopening the tab
    /// shows the saved results instead of the landing Scan button.
    scanned: bool,
    /// The raw search text from the last scan (restored into the search box).
    last_query: String,
    /// The page (0-based) currently shown, so reopening the tab or a row action
    /// returns to the same page rather than jumping back to the first.
    last_page: usize,
    /// The full entry list from the last scan, so the view can be re-rendered on
    /// reopen without re-enumerating the machine.
    last_entries: Vec<Value>,
    /// The last scan, keyed by the row id sent back on a row action, so `about`
    /// / `open_location` can resolve which entry the user acted on.
    last: HashMap<String, Value>,
}

impl Handler for Programs {
    fn capabilities(&self) -> Vec<String> {
        vec!["programs.local".into()]
    }

    fn invoke(
        &mut self,
        _capability: &str,
        method: &str,
        params: Value,
        host: &Host,
    ) -> Result<Value, RpcError> {
        // Optional integration: only offer "Make Report" when a report provider
        // is actually loaded (discovered at call time, never a hard dependency).
        let report = host.has_capability("report.build");
        match method {
            // Landing view: the saved results if the user has scanned this
            // session, otherwise just a Scan button (no enumeration on open).
            "ui" => Ok(if self.scanned {
                let entries = self.last_entries.clone();
                let query = self.last_query.clone();
                self.render(&entries, &query, self.last_page, report)
            } else {
                idle_view()
            }),
            // Scan now (enumerate), save the state, and render (also Refresh).
            "scan" => Ok(self.scan(&params, report)),
            // Turn to a page (from a pager button) without re-enumerating.
            "page" => Ok(self.page(&params, report)),
            "list" => Ok(list_programs()),
            // Row actions: open an entry's details, or open where it's installed.
            "about" => Ok(self.about(&params)),
            "open_location" => Ok(self.open_location(&params, host)),
            "reveal" => Ok(self.reveal(&params, host)),
            "copy_path" => Ok(self.copy_path(&params, host)),
            // Report integration (present only while a report provider is loaded).
            "report_config" => Ok(report_config()),
            "make_report" => Ok(self.make_report(&params, host)),
            other => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("programs has no method {other}"),
            )),
        }
    }
}

/// Build one installed-program record in the shared schema. Used by every
/// platform collector.
///
/// - `source` — where it is registered (`registry-uninstall`, `dpkg`, `rpm`,
///   `flatpak`, `snap`, `xdg-desktop`, `local-bin`, `opt`, …).
/// - `version` — the installed version, if known (empty otherwise).
/// - `publisher` — vendor / maintainer / origin, if known.
/// - `location` — the install directory, entry file, or executable path.
/// - `scope` — `system` (all users / machine) or `user`.
pub(crate) fn entry(
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

/// The landing view: nothing is scanned until the user asks. Just a hint and a
/// Scan button that invokes `scan`.
fn idle_view() -> Value {
    window(
        "Programs",
        vec![
            label("Scan this machine for installed programs.").weak(),
            button("Scan", "programs.local", "scan").primary(),
        ],
    )
}

/// A cell value (empty string if the field is missing).
fn cell(d: &Value, key: &str) -> String {
    d.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// The six visible columns for an entry row.
fn row_cells(d: &Value) -> Vec<String> {
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
/// `(menu/button label, host.open target, value)`. `None` when there's nothing
/// to open — some programs record no install path, and package names aren't
/// filesystem paths, so those rows simply get no open action.
fn open_kind(d: &Value) -> Option<(&'static str, &'static str, String)> {
    let location = cell(d, "location");
    if location.is_empty() {
        return None;
    }
    let p = Path::new(&location);
    if p.is_dir() {
        Some(("Open location", "path", location)) // an install folder → file manager
    } else if p.is_file() {
        Some(("Open file", "path", location)) // an executable / .desktop entry
    } else {
        None // a path that doesn't exist, or a bare package name
    }
}

/// Whether the entry is a system package (`dpkg`/`rpm`) — these record no
/// install directory, but a representative path can be resolved on demand from
/// the package's file list.
fn is_package(d: &Value) -> bool {
    matches!(cell(d, "source").as_str(), "rpm" | "dpkg")
}

/// Whether the entry has a path we can reveal / copy — either a recorded
/// `location`, or a package whose files we can query.
fn has_path(d: &Value) -> bool {
    !cell(d, "location").is_empty() || is_package(d)
}

/// The path to act on: the recorded `location`, or — for a package — a
/// representative file from its contents (resolved lazily, so it costs one
/// subprocess only when the user actually clicks). `None` if nothing resolves.
fn resolved_path(d: &Value) -> Option<String> {
    let loc = cell(d, "location");
    if !loc.is_empty() {
        return Some(loc);
    }
    package_path(&cell(d, "source"), &cell(d, "name"))
}

/// The right-click menu for one entry: About; an open action when the location
/// is an actual file/folder on disk; and — whenever a path is available (a
/// recorded location or a resolvable package) — "Show in Explorer" and
/// "Copy path".
fn row_menu_for(d: &Value) -> Vec<MenuItem> {
    let mut items = vec![menu_item("About", "programs.local", "about").open_in_tab()];
    if let Some((label, _, _)) = open_kind(d) {
        items.push(menu_item(label, "programs.local", "open_location"));
    }
    if has_path(d) {
        // Reveal it in the OS file manager (Explorer / Finder / Files), item selected.
        items.push(menu_item("Show in Explorer", "programs.local", "reveal"));
        items.push(menu_item("Copy path", "programs.local", "copy_path"));
    }
    items
}

/// Put `text` on the system clipboard by piping it to the platform's clipboard
/// tool. Returns whether one succeeded (Linux tries Wayland then X11 tools).
fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        return pipe_to("clip", &[], text);
    }
    #[cfg(target_os = "macos")]
    {
        return pipe_to("pbcopy", &[], text);
    }
    #[cfg(target_os = "linux")]
    {
        return pipe_to("wl-copy", &[], text)
            || pipe_to("xclip", &["-selection", "clipboard"], text)
            || pipe_to("xsel", &["--clipboard", "--input"], text);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = text;
        false
    }
}

/// Spawn `cmd args` and write `text` to its stdin — the shape every clipboard
/// tool takes. `false` if the tool is absent or exits non-zero.
#[allow(dead_code)]
fn pipe_to(cmd: &str, args: &[&str], text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

/// The "Make Report" configuration view (opened in a tab).
fn report_config() -> Value {
    let opts = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    window(
        "Make Report",
        vec![
            label("Report options").strong(),
            select("format", opts(&["In-app view", "Markdown", "HTML", "CSV"])).label("Output"),
            select("content", opts(&["Tables and charts", "Tables only", "Charts only"]))
                .label("Include"),
            select("scope", opts(&["All programs", "System only", "User only"])).label("Show"),
            button("Generate", "programs.local", "make_report").primary().open_in_tab(),
        ],
    )
}

/// The bottom pager: `‹ Prev`, page numbers, and `Next ›`. A run of [`PAGER_WIN`]
/// consecutive pages slides with the current page but stays clamped to the ends,
/// with page 1 and the last page always shown and `…` bridging any gap:
///
/// - start:  `1 2 3 4 5 … 65 ›`
/// - middle: `‹ 1 … 33 34 35 36 37 … 65 ›`
/// - end:    `‹ 1 … 61 62 63 64 65`
///
/// Each number calls `page` with its target; the search box value rides along, so
/// paging stays within the current filter. The current page is highlighted.
fn page_buttons(page: usize, page_count: usize) -> Vec<Widget> {
    const PAGER_WIN: usize = 5;
    let last = page_count - 1;
    let btn = |p: usize, text: String| button(text, "programs.local", "page").args(json!({ "page": p }));

    // A PAGER_WIN-wide run centred on `page`, shifted to stay inside [0, last].
    let lo = page.saturating_sub(PAGER_WIN / 2);
    let hi = (lo + PAGER_WIN - 1).min(last);
    let lo = hi.saturating_sub(PAGER_WIN - 1);

    // The pages to show: first, the sliding run, and last (deduped + ordered).
    let mut shown: Vec<usize> = Vec::new();
    shown.push(0);
    shown.extend(lo..=hi);
    shown.push(last);
    shown.sort_unstable();
    shown.dedup();

    let mut out: Vec<Widget> = Vec::new();
    if page > 0 {
        out.push(btn(page - 1, "‹ Prev".into()));
    }
    let mut prev: Option<usize> = None;
    for p in shown {
        if let Some(pp) = prev {
            if p > pp + 1 {
                out.push(label("…").weak());
            }
        }
        let b = btn(p, (p + 1).to_string());
        out.push(if p == page { b.primary() } else { b });
        prev = Some(p);
    }
    if page + 1 < page_count {
        out.push(btn(page + 1, "Next ›".into()));
    }
    out
}

impl Programs {
    /// Enumerate the machine, save the scan state (so reopening the tab restores
    /// it), and render. `params.query` filters; Refresh calls this again.
    fn scan(&mut self, params: &Value, report: bool) -> Value {
        let query = params.get("query").and_then(Value::as_str).unwrap_or("").to_string();
        let data = list_programs();
        let entries: Vec<Value> = data
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        self.scanned = true;
        self.last_entries = entries.clone();
        self.last_query = query.clone();
        // A fresh scan / search resets to the first page.
        self.render(&entries, &query, 0, report)
    }

    /// Turn to another page of the current (already-scanned) results. Re-slices
    /// `last_entries` — no re-enumeration — honoring the live search box so paging
    /// stays within the filtered set. `params`: `{ query, page }`.
    fn page(&mut self, params: &Value, report: bool) -> Value {
        let query = params.get("query").and_then(Value::as_str).unwrap_or("").to_string();
        let page = params.get("page").and_then(Value::as_u64).unwrap_or(0) as usize;
        let entries = self.last_entries.clone();
        self.last_query = query.clone();
        self.render(&entries, &query, page, report)
    }

    /// The results view: a search box + Refresh (+ Make Report when a report
    /// provider is loaded), then one interactive table of every installed
    /// program. Filters `entries` by `query_raw` and caches each shown entry by
    /// its row id so row actions (`about` / `open_location`) resolve it.
    fn render(&mut self, entries: &[Value], query_raw: &str, page: usize, report: bool) -> Value {
        let query = query_raw.to_lowercase();
        let matches =
            |d: &Value| query.is_empty() || row_cells(d).join(" ").to_lowercase().contains(&query);

        // Filter first, keeping each entry's original index so row ids stay stable
        // across pages (about/open_location resolve by that index).
        let filtered: Vec<(usize, &Value)> =
            entries.iter().enumerate().filter(|(_, d)| matches(d)).collect();
        let total = filtered.len();
        let page_count = if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE };
        let page = page.min(page_count - 1);
        self.last_page = page;
        let start = page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(total);

        // Only the current page is shown, so cache only its rows for row actions.
        self.last.clear();
        let (mut rows, mut ids, mut menus) = (Vec::new(), Vec::new(), Vec::new());
        for (orig, d) in &filtered[start..end] {
            let rid = orig.to_string();
            self.last.insert(rid.clone(), (*d).clone());
            ids.push(rid);
            rows.push(row_cells(d));
            menus.push(row_menu_for(d)); // per-row: open action only when openable
        }

        let cols: Vec<String> = ["Name", "Version", "Publisher", "Source", "Scope", "Location"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut actions = vec![button("Refresh", "programs.local", "scan").primary()];
        if report {
            actions.push(button("Make Report", "programs.local", "report_config").open_in_tab());
        }

        let shown = if total == 0 {
            "no matches".to_string()
        } else {
            format!("showing {}–{} of {} · page {}/{}", start + 1, end, total, page + 1, page_count)
        };

        let mut widgets = vec![
            text("query")
                .label("Search")
                .placeholder("name, version, publisher, source, location…")
                .default(query_raw.to_string()),
            row(actions),
            label("Right-click a row for actions; double-click to open its details.").weak(),
            separator(),
            label(format!("Installed programs ({total}) — {shown}")).strong(),
            table(cols, rows)
                .row_ids(ids)
                .row_menus(menus)
                .on_activate("programs.local", "about"),
        ];
        // Pager at the bottom: page buttons when there's more than one page.
        if page_count > 1 {
            widgets.push(row(page_buttons(page, page_count)));
        }
        window("Programs", widgets)
    }

    /// A detail view for one entry (opened in a new tab from a row action).
    fn about(&self, params: &Value) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        let Some(d) = self.last.get(id) else {
            return window(
                "Program",
                vec![label("This entry isn't in the latest scan — re-scan and try again.").weak()],
            );
        };
        let shown = |v: String| if v.is_empty() { "—".to_string() } else { v };
        let field = |name: &str, val: String| {
            row(vec![label(name.to_string()).strong(), label(shown(val))])
        };
        let title = cell(d, "name");

        let mut widgets = vec![
            label(title.clone()).strong(),
            separator(),
            field("Name", cell(d, "name")),
            field("Version", cell(d, "version")),
            field("Publisher", cell(d, "publisher")),
            field("Source", cell(d, "source")),
            field("Scope", cell(d, "scope")),
            field("Location", cell(d, "location")),
        ];
        // Actions: open where it's installed (when that resolves on disk) and
        // copy the path (whenever one is recorded).
        let mut actions = Vec::new();
        if let Some((label, _, _)) = open_kind(d) {
            actions.push(
                button(label, "programs.local", "open_location").args(json!({ "id": id })).primary(),
            );
        }
        if has_path(d) {
            actions.push(
                button("Show in Explorer", "programs.local", "reveal").args(json!({ "id": id })),
            );
            actions.push(button("Copy path", "programs.local", "copy_path").args(json!({ "id": id })));
        }
        if !actions.is_empty() {
            widgets.push(separator());
            widgets.push(row(actions));
        }
        window(title, widgets)
    }

    /// Open where a program is installed: the folder or file in the file manager.
    /// `params`: `{ id }`.
    fn open_location(&self, params: &Value, host: &Host) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(d) = self.last.get(id) {
            if let Some((_, target, value)) = open_kind(d) {
                host.open(target, &value);
            }
        }
        Value::Null
    }

    /// Reveal the entry's location in the OS file manager (Explorer / Finder /
    /// Files) with the item selected. `params`: `{ id }`.
    fn reveal(&self, params: &Value, host: &Host) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(path) = self.last.get(id).and_then(resolved_path) {
            host.open("reveal", &path);
        }
        Value::Null
    }

    /// Copy the entry's path (install location / entry file, or a resolved
    /// package file) to the system clipboard. `params`: `{ id }`.
    fn copy_path(&self, params: &Value, host: &Host) -> Value {
        let id = params.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(path) = self.last.get(id).and_then(resolved_path) {
            if copy_to_clipboard(&path) {
                host.log(&format!("programs: copied path to clipboard: {path}"));
            } else {
                host.log("programs: couldn't copy path — no clipboard tool found");
            }
        }
        Value::Null
    }

    /// Build a report spec from the last scan and hand it to a report provider.
    fn make_report(&self, params: &Value, host: &Host) -> Value {
        let fmt = match params.get("format").and_then(Value::as_str).unwrap_or("") {
            "Markdown" => "markdown",
            "HTML" => "html",
            "CSV" => "csv",
            _ => "view",
        };
        let content = params.get("content").and_then(Value::as_str).unwrap_or("");
        let scope = params.get("scope").and_then(Value::as_str).unwrap_or("");
        let spec = self.report_spec(fmt, content, scope);
        match host.call("report.build", "build", spec) {
            Ok(v) if v.get("widgets").is_some() => v,
            Ok(_) => window(
                "Report",
                vec![
                    label("Report exported").strong(),
                    label("The document was generated and opened in your default app.").weak(),
                ],
            ),
            Err(e) => window(
                "Report",
                vec![label("Couldn't build the report").strong(), label(format!("{e}")).weak()],
            ),
        }
    }

    /// Assemble the report spec (a by-source chart + a programs table) from the
    /// last scan, honoring the config choices.
    fn report_spec(&self, fmt: &str, content: &str, scope: &str) -> Value {
        let entries = &self.last_entries;
        let in_scope = |d: &Value| match scope {
            "System only" => cell(d, "scope") == "system",
            "User only" => cell(d, "scope") == "user",
            _ => true,
        };
        let total = entries.len();
        let system = entries.iter().filter(|d| cell(d, "scope") == "system").count();
        let user = entries.iter().filter(|d| cell(d, "scope") == "user").count();

        let mut counts: HashMap<String, i64> = HashMap::new();
        for d in entries.iter().filter(|d| in_scope(d)) {
            *counts.entry(cell(d, "source")).or_default() += 1;
        }
        let mut pairs: Vec<(String, i64)> = counts.into_iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let chart_data: Vec<Value> = pairs
            .iter()
            .map(|(k, v)| json!({ "label": k, "value": v }))
            .collect();

        let cols = ["Name", "Version", "Publisher", "Source", "Scope", "Location"];
        let rows: Vec<Vec<String>> =
            entries.iter().filter(|d| in_scope(d)).map(row_cells).collect();

        let mut charts = Vec::new();
        if content != "Tables only" && !chart_data.is_empty() {
            charts.push(json!({ "title": "Programs by source", "data": chart_data }));
        }
        let mut sections = Vec::new();
        if content != "Charts only" {
            sections.push(json!({ "heading": "Installed programs", "columns": cols, "rows": rows }));
        }

        json!({
            "title": "Installed Programs Report",
            "subtitle": format!("{total} programs · {system} system · {user} user"),
            "format": fmt,
            "summary": [
                format!("Total programs: {total}"),
                format!("System-wide: {system}"),
                format!("User: {user}"),
            ],
            "charts": charts,
            "sections": sections,
        })
    }
}

#[cfg(test)]
mod pagination_tests {
    use super::*;

    fn synth(n: usize) -> Vec<Value> {
        (0..n)
            .map(|i| {
                entry("test", format!("prog{i:04}"), String::new(), String::new(), String::new(), "system")
            })
            .collect()
    }
    fn widgets(v: &Value) -> Vec<Value> {
        v.get("widgets").and_then(Value::as_array).cloned().unwrap_or_default()
    }
    fn table_of(v: &Value) -> Value {
        widgets(v)
            .into_iter()
            .find(|w| w.get("kind").and_then(Value::as_str) == Some("table"))
            .unwrap()
    }
    fn rows_len(v: &Value) -> usize {
        table_of(v).get("rows").and_then(Value::as_array).map(|r| r.len()).unwrap_or(0)
    }
    fn pager_texts(v: &Value) -> Vec<String> {
        widgets(v)
            .into_iter()
            .rev()
            .find(|w| w.get("kind").and_then(Value::as_str) == Some("row"))
            .and_then(|r| r.get("children").and_then(Value::as_array).cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c.get("text").and_then(Value::as_str).map(str::to_string))
            .collect()
    }

    #[test]
    fn slices_pages_of_100() {
        let mut p = Programs::default();
        let entries = synth(250); // 3 pages: 100 / 100 / 50

        // Page index 1 -> items 101..200, ids keep the original indices.
        let v = p.render(&entries, "", 1, false);
        assert_eq!(rows_len(&v), 100);
        assert_eq!(
            table_of(&v).get("row_ids").and_then(Value::as_array).unwrap()[0].as_str(),
            Some("100")
        );
        let pager = pager_texts(&v);
        assert!(pager.iter().any(|s| s == "‹ Prev"));
        assert!(pager.iter().any(|s| s == "Next ›"));

        // Last page -> 50 rows, no Next.
        let v3 = p.render(&entries, "", 2, false);
        assert_eq!(rows_len(&v3), 50);
        assert!(!pager_texts(&v3).iter().any(|s| s == "Next ›"));

        // An out-of-range page clamps to the last page.
        assert_eq!(rows_len(&p.render(&entries, "", 999, false)), 50);
    }

    #[test]
    fn one_page_has_no_pager() {
        let mut p = Programs::default();
        let v = p.render(&synth(30), "", 0, false);
        assert_eq!(rows_len(&v), 30);
        // No pager row (only the actions row), so no page-turn buttons.
        assert!(pager_texts(&v).iter().all(|s| s != "Next ›" && s != "‹ Prev"));
    }

    #[test]
    fn pager_windows_match_spec() {
        let mut p = Programs::default();
        let entries = synth(6500); // 65 pages of 100

        // Start: 1 2 3 4 5 … 65 ›   (no Prev)
        assert_eq!(
            pager_texts(&p.render(&entries, "", 0, false)),
            ["1", "2", "3", "4", "5", "…", "65", "Next ›"]
        );
        // Middle (page index 34 → label 35): ‹ 1 … 33 34 35 36 37 … 65 ›
        assert_eq!(
            pager_texts(&p.render(&entries, "", 34, false)),
            ["‹ Prev", "1", "…", "33", "34", "35", "36", "37", "…", "65", "Next ›"]
        );
        // End: ‹ 1 … 61 62 63 64 65   (no Next)
        assert_eq!(
            pager_texts(&p.render(&entries, "", 64, false)),
            ["‹ Prev", "1", "…", "61", "62", "63", "64", "65"]
        );
    }
}

export_module!(Programs);
