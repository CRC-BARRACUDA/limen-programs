//! Handing the last scan to a report provider.

use crate::*;

/// The "Make Report" configuration view (opened in a tab).
pub(crate) fn report_config() -> Value {
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

/// What is shown when the provider wrote a document rather than a view.
pub(crate) fn exported_view() -> Value {
    window(
        "Report",
        vec![
            label("Report exported").strong(),
            label("The document was generated and opened in your default app.").weak(),
        ],
    )
}

/// What is shown when it could not.
pub(crate) fn report_failed_view(why: String) -> Value {
    window(
        "Report",
        vec![label("Couldn't build the report").strong(), label(why).weak()],
    )
}

/// Assemble the report spec (a by-source chart + a programs table) from the
/// last scan, honoring the config choices.
pub(crate) fn report_spec(entries: &[Value], fmt: &str, content: &str, scope: &str) -> Value {
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

    let rows: Vec<Vec<String>> = entries.iter().filter(|d| in_scope(d)).map(row_cells).collect();

    let mut charts = Vec::new();
    if content != "Tables only" && !chart_data.is_empty() {
        charts.push(json!({ "title": "Programs by source", "data": chart_data }));
    }
    let mut sections = Vec::new();
    if content != "Charts only" {
        sections.push(json!({
            "heading": "Installed programs",
            "columns": columns(),
            "rows": rows,
        }));
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
