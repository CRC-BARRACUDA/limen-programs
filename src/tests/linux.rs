//! What this machine reports about itself.

use super::*;

/// Whatever is installed here, the envelope every collector fills is the same —
/// that shape is what other modules read, so it is checked against the real
/// machine rather than a fixture.
#[test]
fn envelope_shape_is_consistent() {
    let v = list_programs();
    assert_eq!(v.get("os").and_then(|o| o.as_str()), Some("linux"));
    let entries = v.get("entries").and_then(|e| e.as_array()).expect("entries[]");
    let total = v.get("total").and_then(|t| t.as_u64()).unwrap();
    assert_eq!(total, entries.len() as u64);
    for e in entries {
        for key in ["source", "name", "version", "publisher", "location", "scope"] {
            assert!(e.get(key).and_then(|x| x.as_str()).is_some(), "missing {key}");
        }
    }
}
