//! What the registry says, made into what the screen can use.

use crate::windows::location;
use crate::*;

/// An install location written in quotes is the path inside them.
#[test]
fn a_quoted_install_location_is_the_path_inside_the_quotes() {
    assert_eq!(
        location(r#""C:\Program Files (x86)\Microsoft Visual Studio\Installer""#),
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer"
    );
    // Padded, inside the quotes or out.
    assert_eq!(location(r#"  "C:\Tools\App "  "#), r"C:\Tools\App");
    // Opened and never closed: still not part of the path.
    assert_eq!(location(r#""C:\Tools\App"#), r"C:\Tools\App");
}

/// One written plainly is left as it is — spaces, trailing slash and all.
#[test]
fn a_plain_install_location_is_left_alone() {
    for path in [r"C:\Program Files\7-Zip\", r"C:\Users\Олег\AppData\Local\Програма", ""] {
        assert_eq!(location(path), path);
    }
}

/// On a real machine: nothing the collector reports is still in quotes.
#[test]
fn no_program_on_this_machine_is_listed_with_a_quoted_location() {
    let listed = list_programs();
    let entries = listed.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    assert!(!entries.is_empty(), "a Windows machine with nothing installed");
    for e in &entries {
        let at = e.get("location").and_then(Value::as_str).unwrap_or("");
        assert!(!at.contains('"'), "{at:?} is still quoted");
    }
}
