//! Handing the last scan to a report provider.
//!
//! The form's choices are shown translated but mean something fixed, and the
//! host sends back the *label* it displayed — so each choice is resolved by
//! finding that label in the very list this module rendered ([`choice`]), never
//! by comparing against English.

use crate::*;

/// What the report provider is asked for: always its preview. What the
/// document is — a PDF — the ground it is printed on, and whether it is signed
/// are decided there, beside the thing being saved, by the module that owns
/// the question, rather than in a dropdown here that has to be kept in step
/// with what a provider can actually produce.
pub(crate) const FORMAT: &str = "view";

/// This machine's name, for the report's subtitle and its filename.
///
/// Read from the kernel on Linux and from the environment on Windows — no new
/// permission, and no process spawned for a string the system already has.
/// Empty rather than a guess if neither answers: a report labelled "unknown" is
/// one somebody has to open to identify.
fn hostname() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(name) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    for var in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(name) = std::env::var(var) {
            if !name.trim().is_empty() {
                return name.trim().to_string();
            }
        }
    }
    String::new()
}

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
            select("content", options(lang, &CONTENT)).label(t("report.include")),
            select("scope", options(lang, &SCOPES)).label(t("report.show")),
            // Not `open_in_tab`: the report provider answers with a pop-up,
            // and one opened into a tab of its own leaves that tab empty.
            button(t("report.generate"), "programs.local", "make_report").primary(),
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
pub(crate) fn report_spec(lang: &str, entries: &[Value], content: Content, scope: Scope) -> Value {
    report_spec_for(lang, entries, content, scope, &hostname())
}

/// [`report_spec`] for the machine named `host` ("" when it has no name to give).
pub(crate) fn report_spec_for(
    lang: &str,
    entries: &[Value],
    content: Content,
    scope: Scope,
    host: &str,
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

    // The machine this is about, in the line under the title and in the name
    // the file is offered under. The report module adds the date; a folder of
    // files all called "programs" is one nobody can find anything in.
    let subtitle = t(if host.is_empty() { "report.doc.subtitle" } else { "report.doc.subtitle_host" });
    let subtitle = n(n(n(subtitle, "{total}", total), "{system}", system), "{user}", user).replace("{host}", host);
    let file_name = if host.is_empty() { "programs".to_string() } else { format!("{host}_programs") };

    json!({
        "title": t("report.doc.title"),
        "subtitle": subtitle,
        "file_name": file_name,
        "format": FORMAT,
        "summary": [
            n(t("report.doc.total"), "{total}", total),
            n(t("report.doc.system"), "{system}", system),
            n(t("report.doc.user"), "{user}", user),
        ],
        "charts": charts,
        "sections": sections,
    })
}
