//! Putting a path where the user can paste it.
//!
//! Every desktop has a clipboard and none of them share an API, so this shells
//! out to whatever the platform provides — and says whether one answered, so the
//! caller can tell the user rather than fail in silence.

/// Put `text` on the system clipboard by piping it to the platform's clipboard
/// tool. Returns whether one succeeded (Linux tries Wayland then X11 tools).
///
/// One `cfg` arm compiles and the rest vanish, so which one is *last* depends on
/// the platform — each has to return rather than fall out of the block as a tail.
#[allow(clippy::needless_return)]
pub(crate) fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        return pipe_to("clip", &[], text);
    }
    #[cfg(target_os = "macos")]
    {
        return pipe_to("pbcopy", &[], text);
    }
    #[cfg(target_os = "linux")]
    {
        return pipe_to("wl-copy", &[], text)
            || pipe_to("xclip", &["-selection", "clipboard"], text)
            || pipe_to("xsel", &["--clipboard", "--input"], text);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = text;
        false
    }
}

/// Spawn `cmd args` and write `text` to its stdin — the shape every clipboard
/// tool takes. `false` if the tool is absent or exits non-zero.
#[allow(dead_code)]
fn pipe_to(cmd: &str, args: &[&str], text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}
