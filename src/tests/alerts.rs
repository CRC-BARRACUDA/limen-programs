//! What the module says in the corner, and when it says nothing.

use super::*;

/// Acting on a row the user can see should never replace what they were
/// looking at — the answer is the same screen, plus a word about what happened.
#[test]
fn a_row_action_answers_with_the_screen_it_came_from() {
    let mut p = Programs::default();
    let args = json!({ "id": "0" });

    let copied = p.acted("en", &args, false, true, "notice.copied", "notice.no_clipboard");
    assert_eq!(
        notice_of(&copied),
        Some(("ok".into(), "Path copied to the clipboard.".into()))
    );
    // Still the table, not some acknowledgement screen of its own.
    assert!(widgets(&copied)
        .iter()
        .any(|w| w.get("kind").and_then(Value::as_str) == Some("table")));

    let failed = p.acted("en", &args, false, false, "notice.copied", "notice.no_clipboard");
    let (level, text) = notice_of(&failed).expect("a failure is worth saying");
    assert_eq!(level, "error");
    assert!(text.contains("clipboard"), "{text}");
}

/// Opening a file manager shows itself. Saying "opened" on top of a window that
/// just appeared is noise, so only the case where nothing opened speaks.
#[test]
fn what_succeeds_visibly_stays_quiet() {
    let mut p = Programs::default();
    let args = json!({ "id": "0" });

    let opened = p.acted("en", &args, false, true, "", "notice.nothing_to_open");
    assert_eq!(notice_of(&opened), None);

    let nothing = p.acted("en", &args, false, false, "", "notice.nothing_to_open");
    assert_eq!(notice_of(&nothing).map(|(l, _)| l), Some("error".into()));
}

/// Row ids are positions in the last scan, so one from an older scan can point
/// at nothing. That is worth saying rather than opening a blank tab.
#[test]
fn an_entry_that_is_gone_says_so() {
    let mut p = Programs::default();
    let v = p.about("en", &json!({ "id": "4242" }), Screen::Modal);
    let (level, text) = notice_of(&v).expect("a stale row is worth saying");
    assert_eq!(level, "error");
    assert!(!text.contains("notice."), "the key leaked: {text}");
}

/// An action taken from the entry's own screen comes back to that screen — the
/// tab stays a tab, and the pop-up stays open over the table it was opened from.
#[test]
fn an_action_answers_on_the_screen_it_was_taken_from() {
    for (from, is_modal) in [("tab", false), ("modal", true)] {
        let mut p = Programs::default();
        let v = p.acted(
            "en",
            &json!({ "id": "4242", "from": from }),
            false,
            true,
            "notice.copied",
            "notice.no_clipboard",
        );
        // No table: this is the entry's own screen, not the results.
        assert!(!widgets(&v)
            .iter()
            .any(|w| w.get("kind").and_then(Value::as_str) == Some("table")));
        assert_eq!(v.get("modal").is_some(), is_modal, "from {from}: {v}");
    }
}

/// The count a scan reports is filled in, in whichever language asked.
#[test]
fn the_scan_notice_carries_its_count() {
    for (lang, digits) in [("en", "17"), ("uk", "17")] {
        let said = catalog().tr(lang, "notice.scanned").replace("{total}", digits);
        assert!(said.contains(digits), "{lang}: {said}");
        assert!(!said.contains("{total}"), "{lang}: placeholder left: {said}");
    }
}
