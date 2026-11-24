//! That a path copied is the path pasted, whatever it is written in.

use crate::clipboard::win32;
use crate::copy_to_clipboard;

/// The case that was broken: `clip.exe` was handed UTF-8 and read it in the
/// console's code page, so every letter outside ASCII came out as two others.
///
/// Ignored because it has to use the real clipboard: what is on it when the
/// test starts is put back if it was text, and lost if it was anything else.
#[test]
#[ignore = "overwrites the clipboard; run with --ignored"]
fn a_path_comes_back_from_the_clipboard_as_it_went_in() {
    let before = win32::text();

    for path in [
        r"C:\Program Files\7-Zip",
        r"C:\Users\Олег\Програми\Тест",
        r"D:\ソフト\日本語",
        r"C:\Temp\naïve — café 😀",
        "",
    ] {
        assert!(copy_to_clipboard(path), "{path:?} was not taken");
        assert_eq!(win32::text().as_deref(), Some(path));
    }

    if let Some(text) = before {
        win32::set_text(&text);
    }
}
