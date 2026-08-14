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
        about_view(lang, &entries[0], "0"),
        stale_view(lang),
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
        let shown = options(lang, &FORMATS);
        assert_eq!(choice(lang, &FORMATS, &shown[1]), "markdown");
        assert_eq!(choice(lang, &FORMATS, &shown[3]), "csv");

        let scope = choice(lang, &SCOPES, &options(lang, &SCOPES)[2]);
        assert!(matches!(scope, Scope::User));
        let content = choice(lang, &CONTENT, &options(lang, &CONTENT)[1]);
        assert!(matches!(content, Content::Tables));
    }
    // Something we never offered is nobody's choice: fall back to the first.
    assert_eq!(choice("en", &FORMATS, "PDF, surely"), "view");
    // And an English label sent while the UI is Ukrainian is not a Ukrainian
    // choice — the form only ever sends back what it was given.
    assert!(matches!(choice("uk", &SCOPES, "System only"), Scope::All));
}

/// The document a report provider is handed is written in the user's language
/// too — it is the same run, not a separate English artefact.
#[test]
fn the_report_document_speaks_the_active_language() {
    let entries = synth(3);
    let spec = report_spec("uk", &entries, "markdown", Content::All, Scope::All);
    let title = spec.get("title").and_then(Value::as_str).unwrap_or("");
    assert_eq!(title, "Звіт про встановлені програми");
    let summary = spec.get("summary").unwrap().to_string();
    assert!(summary.contains('3'), "{summary}");
    assert!(!summary.contains("{total}"), "placeholder left: {summary}");
    // The columns are headings, so they are translated; `format` is a protocol
    // value, so it is not.
    let s = spec.to_string();
    assert!(s.contains("Видавець"), "columns not translated");
    assert_eq!(spec.get("format").and_then(Value::as_str), Some("markdown"));
}
