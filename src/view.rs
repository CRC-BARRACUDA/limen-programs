//! Everything the module draws.
//!
//! Every screen takes `lang` and resolves its words through the catalog, so the
//! same call renders in whatever language the host is set to. Nothing here holds
//! state: the handler decides *what* to show, these decide how it looks.

use crate::*;

/// The landing view: nothing is scanned until the user asks. Just a hint and a
/// Scan button that invokes `scan`.
pub(crate) fn idle_view(lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    window(
        t("title"),
        vec![
            label(t("idle.hint")).weak(),
            button(t("idle.scan"), "programs.local", "scan").primary(),
        ],
    )
}

/// The table's column headings, in order.
pub(crate) fn columns(lang: &str) -> Vec<String> {
    let t = |k: &str| catalog().tr(lang, k);
    vec![
        t("table.name"),
        t("table.version"),
        t("table.publisher"),
        t("table.source"),
        t("table.scope"),
        t("table.location"),
    ]
}

/// Which screen a row action was taken from — and so which one it answers with.
///
/// The same three actions hang off the table, the details pop-up and the
/// details tab; each has to come back to where the user actually is, so the
/// screen rides along in the call rather than being guessed at.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Screen {
    Table,
    Modal,
    Tab,
}

impl Screen {
    /// Where a call came from. Absent means the table — that is the screen the
    /// row menu belongs to, and the one an older call would have meant.
    pub(crate) fn of(params: &Value) -> Screen {
        match params.get("from").and_then(Value::as_str) {
            Some("modal") => Screen::Modal,
            Some("tab") => Screen::Tab,
            _ => Screen::Table,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Screen::Table => "table",
            Screen::Modal => "modal",
            Screen::Tab => "tab",
        }
    }
}

/// The details pop-up's identity. One at a time: opening another entry redraws
/// this pop-up rather than stacking a second one over it.
const ABOUT_MODAL: &str = "programs.about";

/// How wide the details pop-up asks to be. Wide enough for an install path,
/// which is the longest thing on it; the host clamps it to the window.
const ABOUT_WIDTH: f32 = 560.0;

/// The right-click menu for one entry: About (a pop-up) and the same in a new
/// tab; an open action when the location is an actual file/folder on disk; and
/// — whenever a path is available (a recorded location or a resolvable package)
/// — "Show in Explorer" and "Copy path".
pub(crate) fn row_menu_for(lang: &str, d: &Value) -> Vec<MenuItem> {
    let t = |k: &str| catalog().tr(lang, k);
    let mut items = vec![
        menu_item(t("menu.about"), "programs.local", "about"),
        menu_item(t("menu.about_tab"), "programs.local", "about_tab").open_in_tab(),
    ];
    if let Some((key, _, _)) = open_kind(d) {
        items.push(menu_item(t(key), "programs.local", "open_location"));
    }
    if has_path(d) {
        // Reveal it in the OS file manager (Explorer / Finder / Files), item selected.
        items.push(menu_item(t("menu.reveal"), "programs.local", "reveal"));
        items.push(menu_item(t("menu.copy_path"), "programs.local", "copy_path"));
    }
    items
}

/// One page of results: the search box, the actions, and the table itself.
///
/// The rows are already filtered, sliced and cached by the caller — this only
/// puts them on screen, so paging and searching cost no re-enumeration.
#[allow(clippy::too_many_arguments)]
pub(crate) fn results_view(
    lang: &str,
    query_raw: &str,
    rows: Vec<Vec<String>>,
    ids: Vec<String>,
    menus: Vec<Vec<MenuItem>>,
    (start, end, total): (usize, usize, usize),
    (page, page_count): (usize, usize),
    report: bool,
) -> Value {
    let t = |k: &str| catalog().tr(lang, k);

    let mut actions = vec![button(t("view.refresh"), "programs.local", "scan").primary()];
    if report {
        actions.push(
            button(t("view.make_report"), "programs.local", "report_config").open_in_tab(),
        );
    }

    let shown = if total == 0 {
        t("view.no_matches")
    } else {
        t("view.shown")
            .replace("{from}", &(start + 1).to_string())
            .replace("{to}", &end.to_string())
            .replace("{total}", &total.to_string())
            .replace("{page}", &(page + 1).to_string())
            .replace("{pages}", &page_count.to_string())
    };
    let heading = t("view.heading")
        .replace("{total}", &total.to_string())
        .replace("{shown}", &shown);

    let mut widgets = vec![
        text("query")
            .label(t("view.search"))
            .placeholder(t("view.search_hint"))
            .default(query_raw.to_string()),
        row(actions),
        label(t("view.row_hint")).weak(),
        separator(),
        label(heading).strong(),
        // Double-click opens the details *here*, as a pop-up over the table —
        // `on_activate` would open a tab instead, and the pop-up is what a
        // second look at one row wants: read it, close it, carry on.
        table(columns(lang), rows)
            .row_ids(ids)
            .row_menus(menus)
            .on_activate_here("programs.local", "about"),
    ];
    // Pager at the bottom: page buttons when there's more than one page.
    if page_count > 1 {
        widgets.push(row(page_buttons(lang, page, page_count)));
    }
    window(t("title"), widgets)
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
pub(crate) fn page_buttons(lang: &str, page: usize, page_count: usize) -> Vec<Widget> {
    const PAGER_WIN: usize = 5;
    let t = |k: &str| catalog().tr(lang, k);
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
        out.push(btn(page - 1, t("view.prev")));
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
        out.push(btn(page + 1, t("view.next")));
    }
    out
}

/// One entry in full — the same information whether it is read in a pop-up over
/// the table (double-click, or **About**) or in a tab of its own.
///
/// Its actions carry `where_` back, so when one of them answers with a notice it
/// redraws *this* screen rather than dropping the user somewhere else.
pub(crate) fn about_view(lang: &str, d: &Value, id: &str, where_: Screen) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    let shown = |v: String| if v.is_empty() { t("about.unknown") } else { v };
    let field = |name: String, val: String| row(vec![label(name).strong(), label(shown(val))]);
    let title = cell(d, "name");
    let args = json!({ "id": id, "from": where_.tag() });

    let mut widgets = vec![
        label(title.clone()).strong(),
        separator(),
        field(t("table.name"), cell(d, "name")),
        field(t("table.version"), cell(d, "version")),
        field(t("table.publisher"), cell(d, "publisher")),
        field(t("table.source"), cell(d, "source")),
        field(t("table.scope"), cell(d, "scope")),
        field(t("table.location"), cell(d, "location")),
    ];
    // Actions: open where it's installed (when that resolves on disk) and
    // copy the path (whenever one is recorded).
    let mut actions = Vec::new();
    if let Some((key, _, _)) = open_kind(d) {
        actions.push(
            button(t(key), "programs.local", "open_location")
                .args(args.clone())
                .primary(),
        );
    }
    if has_path(d) {
        actions.push(
            button(t("menu.reveal"), "programs.local", "reveal").args(args.clone()),
        );
        actions.push(button(t("menu.copy_path"), "programs.local", "copy_path").args(args));
    }
    // A pop-up needs a way out that the user would think to press; Esc alone is
    // not one. In a tab the tab's own close is that way out.
    if where_ == Screen::Modal {
        actions.push(button(t("about.close"), "programs.local", "about").dismiss());
    }
    if !actions.is_empty() {
        widgets.push(separator());
        widgets.push(row(actions));
    }
    match where_ {
        Screen::Tab => window(title, widgets),
        _ => window_modal_sized(title, ABOUT_MODAL, ABOUT_WIDTH, widgets),
    }
}

/// What a row action opens when the entry it names is no longer in the scan —
/// the results are re-enumerated on every scan, and ids are positions in that
/// list, so an id from an older scan can point at nothing.
pub(crate) fn stale_view(lang: &str, where_: Screen) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    let mut widgets = vec![label(t("about.stale")).weak()];
    match where_ {
        Screen::Tab => window(t("about.title"), widgets),
        _ => {
            widgets.push(button(t("about.close"), "programs.local", "about").dismiss());
            window_modal_sized(t("about.title"), ABOUT_MODAL, ABOUT_WIDTH, widgets)
        }
    }
}
