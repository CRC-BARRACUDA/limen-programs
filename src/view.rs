//! Everything the module draws.
//!
//! Nothing here holds state: the handler decides *what* to show, these decide
//! how it looks.

use crate::*;

/// The landing view: nothing is scanned until the user asks. Just a hint and a
/// Scan button that invokes `scan`.
pub(crate) fn idle_view() -> Value {
    window(
        "Programs",
        vec![
            label("Scan this machine for installed programs.").weak(),
            button("Scan", "programs.local", "scan").primary(),
        ],
    )
}

/// The table's column headings, in order.
pub(crate) fn columns() -> Vec<String> {
    ["Name", "Version", "Publisher", "Source", "Scope", "Location"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// The right-click menu for one entry: About; an open action when the location
/// is an actual file/folder on disk; and — whenever a path is available (a
/// recorded location or a resolvable package) — "Show in Explorer" and
/// "Copy path".
pub(crate) fn row_menu_for(d: &Value) -> Vec<MenuItem> {
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

/// One page of results: the search box, the actions, and the table itself.
///
/// The rows are already filtered, sliced and cached by the caller — this only
/// puts them on screen, so paging and searching cost no re-enumeration.
pub(crate) fn results_view(
    query_raw: &str,
    rows: Vec<Vec<String>>,
    ids: Vec<String>,
    menus: Vec<Vec<MenuItem>>,
    (start, end, total): (usize, usize, usize),
    (page, page_count): (usize, usize),
    report: bool,
) -> Value {
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
        table(columns(), rows)
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
pub(crate) fn page_buttons(page: usize, page_count: usize) -> Vec<Widget> {
    const PAGER_WIN: usize = 5;
    let last = page_count - 1;
    let btn =
        |p: usize, text: String| button(text, "programs.local", "page").args(json!({ "page": p }));

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

/// A detail view for one entry (opened in a new tab from a row action).
pub(crate) fn about_view(d: &Value, id: &str) -> Value {
    let shown = |v: String| if v.is_empty() { "—".to_string() } else { v };
    let field = |name: &str, val: String| row(vec![label(name.to_string()).strong(), label(shown(val))]);
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
            button(label, "programs.local", "open_location")
                .args(json!({ "id": id }))
                .primary(),
        );
    }
    if has_path(d) {
        actions.push(button("Show in Explorer", "programs.local", "reveal").args(json!({ "id": id })));
        actions.push(button("Copy path", "programs.local", "copy_path").args(json!({ "id": id })));
    }
    if !actions.is_empty() {
        widgets.push(separator());
        widgets.push(row(actions));
    }
    window(title, widgets)
}

/// What a row action opens when the entry it names is no longer in the scan —
/// the results are re-enumerated on every scan, and ids are positions in that
/// list, so an id from an older scan can point at nothing.
pub(crate) fn stale_view() -> Value {
    window(
        "Program",
        vec![label("This entry isn't in the latest scan — re-scan and try again.").weak()],
    )
}
