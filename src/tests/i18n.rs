//! Both languages say the same things, and neither leaks the other.

use super::*;

/// Every screen the module can draw, in one language.
fn every_view(lang: &str) -> Vec<Value> {
    let mut p = Programs::default();
    let entries = synth(250);
    vec![
        idle_view(lang),
        p.render(lang, &entries, "", 1, true),
        p.render(lang, &entries, "nothing-matches-this", 0, false),
        about_view(lang, &entries[0], "0", Screen::Modal),
        about_view(lang, &entries[0], "0", Screen::Tab),
        stale_view(lang, Screen::Modal),
        stale_view(lang, Screen::Tab),
        report_config(lang),
        exported_view(lang),
        report_failed_view(lang, "boom".into()),
    ]
}

#[test]
fn ukrainian_covers_every_english_key() {
    let en = catalog_keys(include_str!("../../resources/locales/en.toml"));
    let uk = catalog_keys(include_str!("../../resources/locales/uk.toml"));
    let missing: Vec<&String> = en.iter().filter(|k| !uk.contains(k)).collect();
    assert!(missing.is_empty(), "Ukrainian is missing: {missing:?}");
}

/// A key that resolves to itself is in no catalog at all — the fallback chain is
/// lang -> en -> the key, so that is how a typo surfaces.
#[test]
fn no_rendered_key_falls_through_to_itself() {
    let keys = catalog_keys(include_str!("../../resources/locales/en.toml"));
    for lang in ["en", "uk"] {
        for v in every_view(lang) {
            let s = v.to_string();
            for k in &keys {
                assert!(
                    !s.contains(&format!(":\"{k}\"")),
                    "{lang}: key rendered instead of its translation: {k}"
                );
            }
        }
    }
}

/// Rendering in Ukrainian must not leave English prose behind — a hardcoded
/// literal is not a key, so the leak test above cannot catch it.
#[test]
fn the_ukrainian_views_carry_no_english_prose() {
    const ENGLISH: [&str; 12] = [
        "Installed programs",
        "Scan this machine",
        "Search",
        "Refresh",
        "Make Report",
        "Show in Explorer",
        "Copy path",
        "Report options",
        "All programs",
        "Tables and charts",
        "showing",
        "Publisher",
    ];
    for v in every_view("uk") {
        let s = v.to_string();
        for phrase in ENGLISH {
            assert!(!s.contains(phrase), "English left in a Ukrainian view: {phrase}");
        }
    }
}

/// A report choice is shown translated but means something fixed, and the form
/// answers with the label it showed — so the label has to lead back to the
/// meaning, in whichever language it was picked.
#[test]
fn a_choice_made_in_either_language_means_the_same_thing() {
    for lang in ["en", "uk"] {
        let scope = choice(lang, &SCOPES, &options(lang, &SCOPES)[2]);
        assert!(matches!(scope, Scope::User));
        let content = choice(lang, &CONTENT, &options(lang, &CONTENT)[1]);
        assert!(matches!(content, Content::Tables));
    }
    // Something we never offered is nobody's choice: fall back to the first.
    assert!(matches!(choice("en", &CONTENT, "Everything, surely"), Content::All));
    // And an English label sent while the UI is Ukrainian is not a Ukrainian
    // choice — the form only ever sends back what it was given.
    assert!(matches!(choice("uk", &SCOPES, "System only"), Scope::All));
}

/// The document a report provider is handed is written in the user's language
/// too — it is the same run, not a separate English artefact.
#[test]
fn the_report_document_speaks_the_active_language() {
    let entries = synth(3);
    let spec = report_spec_for("uk", &entries, Content::All, Scope::All, "WS-017");
    let title = spec.get("title").and_then(Value::as_str).unwrap_or("");
    assert_eq!(title, "Звіт про встановлені програми");
    let summary = spec.get("summary").unwrap().to_string();
    assert!(summary.contains('3'), "{summary}");
    assert!(!summary.contains("{total}"), "placeholder left: {summary}");
    // The columns are headings, so they are translated; `format` is a protocol
    // value, so it is not.
    let s = spec.to_string();
    assert!(s.contains("Видавець"), "columns not translated");
    // What is asked for is the provider's preview, where the PDF, its ground
    // and its signature are chosen — never a format picked here.
    assert_eq!(spec.get("format").and_then(Value::as_str), Some("view"));
    // Filed under the machine it is about, which the subtitle names too.
    assert_eq!(spec.get("file_name").and_then(Value::as_str), Some("WS-017_programs"));
    let subtitle = spec.get("subtitle").and_then(Value::as_str).unwrap_or("");
    assert!(subtitle.starts_with("WS-017 · "), "{subtitle}");
    assert!(!subtitle.contains('{'), "placeholder left: {subtitle}");
    // A machine with no name to give: no gap where one would go.
    let anon = report_spec_for("en", &entries, Content::All, Scope::All, "");
    assert_eq!(anon.get("file_name").and_then(Value::as_str), Some("programs"));
    assert!(anon.get("subtitle").and_then(Value::as_str).unwrap().starts_with("3 programs"));
}

/// The report form asks what goes into the document and nothing about what
/// kind of document it is: that is the report provider's preview to ask. And
/// its button does not open a tab — the preview is a pop-up over this one.
#[test]
fn the_report_form_leaves_the_document_to_the_provider() {
    let form = report_config("en");
    let widgets = form["widgets"].as_array().unwrap();
    let ids: Vec<&str> = widgets.iter().filter_map(|w| w.get("id").and_then(Value::as_str)).collect();
    assert_eq!(ids, vec!["content", "scope"], "no format to choose");
    let generate = widgets.iter().find(|w| w["kind"] == "button").expect("the Generate button");
    assert!(!generate.get("open_in_tab").and_then(Value::as_bool).unwrap_or(false));
}
