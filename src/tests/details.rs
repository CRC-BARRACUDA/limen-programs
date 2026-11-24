//! Reading one row: over the table, or in a tab of its own.

use super::*;

/// A scanned table, so row id "0" resolves to something.
fn scanned() -> Programs {
    let mut p = Programs::default();
    p.render("en", &synth(3), "", 0, false);
    p
}

fn widget_texts(v: &Value) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        if let Some(t) = v.get("text").and_then(Value::as_str) {
            out.push(t.to_string());
        }
        for key in ["widgets", "children"] {
            for c in v.get(key).and_then(Value::as_array).into_iter().flatten() {
                walk(c, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

/// Double-clicking a row, and **About**, both open the details over the table
/// rather than replacing it or taking the user to another tab.
#[test]
fn a_second_look_at_a_row_opens_over_the_table() {
    let v = scanned().about("en", &json!({ "id": "0" }), Screen::Modal);
    assert_eq!(
        v.get("modal").and_then(Value::as_str),
        Some("programs.about"),
        "the details should be a pop-up: {v}"
    );
    assert!(v.get("modal_width").is_some(), "a pop-up asks for its width");
    // A pop-up needs a way out the user would think to press.
    assert!(widget_texts(&v).iter().any(|t| t == "Close"), "{v}");
}

/// The same details, asked for in a tab, are a plain screen: a tab closes
/// itself, so a Close button inside it would close nothing.
#[test]
fn the_same_details_in_a_tab_are_not_a_pop_up() {
    let v = scanned().about("en", &json!({ "id": "0" }), Screen::Tab);
    assert!(v.get("modal").is_none(), "a tab is not a pop-up: {v}");
    assert!(!widget_texts(&v).iter().any(|t| t == "Close"));
    // Same entry either way — only the frame differs.
    assert_eq!(v.get("title").and_then(Value::as_str), Some("prog0000"));
}

/// Every row offers both readings, and the tab one is the only one that opens
/// a tab.
#[test]
fn a_row_offers_both_ways_of_reading_it() {
    let v = Programs::default().render("en", &synth(1), "", 0, false);
    let menus = table_of(&v).get("row_menus").cloned().expect("row_menus");
    let items = menus.get(0).and_then(Value::as_array).cloned().expect("the first row's menu");
    let by_method = |m: &str| {
        items
            .iter()
            .find(|i| i.get("action").and_then(|a| a.get("method")).and_then(Value::as_str) == Some(m))
            .cloned()
    };

    let about = by_method("about").expect("About");
    assert_eq!(about.get("label").and_then(Value::as_str), Some("About"));
    assert!(about.get("open_in_tab").is_none(), "About opens a pop-up");

    let tabbed = by_method("about_tab").expect("About in a new tab");
    assert_eq!(tabbed.get("open_in_tab").and_then(Value::as_bool), Some(true));
}

/// Double-click must not open a tab: the table asks for the result *here*, and
/// the pop-up rides on that.
#[test]
fn double_click_opens_it_where_the_table_is() {
    let mut p = Programs::default();
    let v = p.render("en", &synth(3), "", 0, false);
    let activate = table_of(&v).get("on_activate").cloned().expect("on_activate");
    assert_eq!(
        activate.get("action").and_then(|a| a.get("method")).and_then(Value::as_str),
        Some("about")
    );
    assert_eq!(activate.get("open_in_tab").and_then(Value::as_bool), Some(false));
}

/// The details name the entry once, not twice.
///
/// The host draws a view's title itself — in a pop-up's title bar, and as the
/// name of a tab — so a heading of our own repeating it put the same string on
/// screen twice, one line under the other. Program names are the long kind
/// ("Microsoft Visual C++ 2015-2022 Redistributable (x64) - 14.44.35211"), which
/// is precisely when a doubled line is most obvious and least useful.
#[test]
fn the_details_do_not_repeat_their_own_title() {
    for (which, where_) in [("pop-up", Screen::Modal), ("tab", Screen::Tab)] {
        let v = scanned().about("en", &json!({ "id": "0" }), where_);
        let title = v
            .get("title")
            .and_then(Value::as_str)
            .expect("the view names itself")
            .to_string();
        assert!(!title.is_empty(), "no title to compare against: {v}");

        // The name still appears — as the value of the Name field — but never as
        // a heading that simply restates the title.
        let headings = widget_texts(&v).into_iter().filter(|t| *t == title).count();
        assert_eq!(
            headings, 1,
            "{which}: the title is repeated {headings} times inside the view: {v}"
        );
    }
}
