//! What the module is expected to do, in the language of what it is for.
//!
//! A cdylib has no rlib to link an integration test against, so these live in
//! the crate: one file per part, and the fixtures they share here.

use crate::*;

mod alerts;
mod details;
mod i18n;
mod pagination;

#[cfg(target_os = "linux")]
mod linux;

/// `n` entries, named so their order is their index — enough for the view to
/// have something to slice, page and filter.
fn synth(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            entry(
                "test",
                format!("prog{i:04}"),
                String::new(),
                String::new(),
                String::new(),
                "system",
            )
        })
        .collect()
}

fn widgets(v: &Value) -> Vec<Value> {
    v.get("widgets").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn table_of(v: &Value) -> Value {
    widgets(v)
        .into_iter()
        .find(|w| w.get("kind").and_then(Value::as_str) == Some("table"))
        .unwrap()
}

fn rows_len(v: &Value) -> usize {
    table_of(v).get("rows").and_then(Value::as_array).map(|r| r.len()).unwrap_or(0)
}

fn pager_texts(v: &Value) -> Vec<String> {
    widgets(v)
        .into_iter()
        .rev()
        .find(|w| w.get("kind").and_then(Value::as_str) == Some("row"))
        .and_then(|r| r.get("children").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c.get("text").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// The notice a view carries, if any: `(level, text)`.
fn notice_of(v: &Value) -> Option<(String, String)> {
    let n = v.get("notice")?;
    Some((
        n.get("level")?.as_str()?.to_string(),
        n.get("text")?.as_str()?.to_string(),
    ))
}

/// Every `a.b` key in a catalog file, in the flattened form the SDK looks up.
fn catalog_keys(toml: &str) -> Vec<String> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = format!("{name}.");
        } else if let Some((k, _)) = line.split_once('=') {
            out.push(format!("{section}{}", k.trim()));
        }
    }
    out
}
