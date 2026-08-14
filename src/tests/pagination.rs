//! A machine with thousands of programs is shown a page at a time.

use super::*;

#[test]
fn slices_pages_of_100() {
    let mut p = Programs::default();
    let entries = synth(250); // 3 pages: 100 / 100 / 50

    // Page index 1 -> items 101..200, ids keep the original indices.
    let v = p.render("en", &entries, "", 1, false);
    assert_eq!(rows_len(&v), 100);
    assert_eq!(
        table_of(&v).get("row_ids").and_then(Value::as_array).unwrap()[0].as_str(),
        Some("100")
    );
    let pager = pager_texts(&v);
    assert!(pager.iter().any(|s| s == "‹ Prev"));
    assert!(pager.iter().any(|s| s == "Next ›"));

    // Last page -> 50 rows, no Next.
    let v3 = p.render("en", &entries, "", 2, false);
    assert_eq!(rows_len(&v3), 50);
    assert!(!pager_texts(&v3).iter().any(|s| s == "Next ›"));

    // An out-of-range page clamps to the last page.
    assert_eq!(rows_len(&p.render("en", &entries, "", 999, false)), 50);
}

#[test]
fn one_page_has_no_pager() {
    let mut p = Programs::default();
    let v = p.render("en", &synth(30), "", 0, false);
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
        pager_texts(&p.render("en", &entries, "", 0, false)),
        ["1", "2", "3", "4", "5", "…", "65", "Next ›"]
    );
    // Middle (page index 34 → label 35): ‹ 1 … 33 34 35 36 37 … 65 ›
    assert_eq!(
        pager_texts(&p.render("en", &entries, "", 34, false)),
        ["‹ Prev", "1", "…", "33", "34", "35", "36", "37", "…", "65", "Next ›"]
    );
    // End: ‹ 1 … 61 62 63 64 65   (no Next)
    assert_eq!(
        pager_texts(&p.render("en", &entries, "", 64, false)),
        ["‹ Prev", "1", "…", "61", "62", "63", "64", "65"]
    );
}

/// The pager is words as much as arrows, so it turns pages in Ukrainian too.
#[test]
fn the_pager_speaks_the_active_language() {
    let mut p = Programs::default();
    let v = p.render("uk", &synth(250), "", 1, false);
    let pager = pager_texts(&v);
    assert!(pager.iter().any(|s| s == "‹ Назад"), "{pager:?}");
    assert!(pager.iter().any(|s| s == "Далі ›"), "{pager:?}");
    // The page numbers are numbers in every language.
    assert!(pager.iter().any(|s| s == "3"));
}
