//! Handing the last scan to a report provider.
//!
//! The form's choices are shown translated but mean something fixed, and the
//! host sends back the *label* it displayed — so each choice is resolved by
//! finding that label in the very list this module rendered ([`choice`]), never
//! by comparing against English.

use crate::*;

/// The output formats, as `(catalog key, what the report provider is told)`.
pub(crate) const FORMATS: [(&str, &str); 4] = [
    ("report.format.view", "view"),
    ("report.format.markdown", "markdown"),
    ("report.format.html", "html"),
    ("report.format.csv", "csv"),
];

/// What the document carries.
pub(crate) const CONTENT: [(&str, Content); 3] = [
    ("report.content.all", Content::All),
    ("report.content.tables", Content::Tables),
    ("report.content.charts", Content::Charts),
];

/// Which programs it covers.
pub(crate) const SCOPES: [(&str, Scope); 3] = [
    ("report.scope.all", Scope::All),
    ("report.scope.system", Scope::System),
    ("report.scope.user", Scope::User),
];

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Content {
    All,
    Tables,
    Charts,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Scope {
    All,
    System,
    User,
}

/// The labels for a set of choices, in order.
pub(crate) fn options<T: Copy>(lang: &str, choices: &[(&str, T)]) -> Vec<String> {
    choices.iter().map(|(k, _)| catalog().tr(lang, k)).collect()
}

/// Resolve what the form sent back: the label the user picked, matched against
/// the same list that was shown. Falls back to the first choice — a form that
/// answers with something we never offered is a form nobody chose from.
pub(crate) fn choice<T: Copy>(lang: &str, choices: &[(&str, T)], sent: &str) -> T {
    choices
        .iter()
        .find(|(k, _)| catalog().tr(lang, k) == sent)
        .map(|(_, v)| *v)
        .unwrap_or(choices[0].1)
}

/// The "Make Report" configuration view (opened in a tab).
pub(crate) fn report_config(lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    window(
        t("report.title"),
        vec![
            label(t("report.options")).strong(),
            select("format", options(lang, &FORMATS)).label(t("report.output")),
            select("content", options(lang, &CONTENT)).label(t("report.include")),
            select("scope", options(lang, &SCOPES)).label(t("report.show")),
            button(t("report.generate"), "programs.local", "make_report")
                .primary()
                .open_in_tab(),
        ],
    )
}

/// What is shown when the provider wrote a document rather than a view.
pub(crate) fn exported_view(lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    window(
        t("report.result_title"),
        vec![
            label(t("report.exported")).strong(),
            label(t("report.exported_note")).weak(),
        ],
    )
}

/// What is shown when it could not.
pub(crate) fn report_failed_view(lang: &str, why: String) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    window(
        t("report.result_title"),
        vec![label(t("report.failed")).strong(), label(why).weak()],
    )
}

/// Assemble the report spec (a by-source chart + a programs table) from the
/// last scan, honoring the config choices.
pub(crate) fn report_spec(
    lang: &str,
    entries: &[Value],
    fmt: &str,
    content: Content,
    scope: Scope,
) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    let in_scope = |d: &Value| match scope {
        Scope::System => cell(d, "scope") == "system",
        Scope::User => cell(d, "scope") == "user",
        Scope::All => true,
    };
    let total = entries.len();
    let system = entries.iter().filter(|d| cell(d, "scope") == "system").count();
    let user = entries.iter().filter(|d| cell(d, "scope") == "user").count();
    let n = |s: String, k: &str, v: usize| s.replace(k, &v.to_string());

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

    let rows: Vec<Vec<String>> = entries.iter().filter(|d| in_scope(d)).map(row_cells).collect();

    let mut charts = Vec::new();
    if content != Content::Tables && !chart_data.is_empty() {
        charts.push(json!({ "title": t("report.doc.chart_by_source"), "data": chart_data }));
    }
    let mut sections = Vec::new();
    if content != Content::Charts {
        sections.push(json!({
            "heading": t("report.doc.section"),
            "columns": columns(lang),
            "rows": rows,
        }));
    }

    let subtitle = n(
        n(n(t("report.doc.subtitle"), "{total}", total), "{system}", system),
        "{user}",
        user,
    );

    json!({
        "title": t("report.doc.title"),
        "subtitle": subtitle,
        "format": fmt,
        "summary": [
            n(t("report.doc.total"), "{total}", total),
            n(t("report.doc.system"), "{system}", system),
            n(t("report.doc.user"), "{user}", user),
        ],
        "charts": charts,
        "sections": sections,
    })
}
