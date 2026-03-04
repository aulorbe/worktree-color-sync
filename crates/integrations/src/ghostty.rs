use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use tracing::warn;

/// Ghostty tab/surface update strategy:
/// - Worktree context: set OSC 11 background + OSC 10 foreground for contrast.
/// - Non-worktree context: reset dynamic colors back to terminal defaults.
/// This is naturally tab-specific because each tab has its own PTY/TTY.
pub fn apply_background_color_to_tty(terminal_id: &str, color_hex: &str) -> Result<()> {
    if !terminal_id.starts_with("/dev/") {
        anyhow::bail!("terminal_id must be a tty path like /dev/ttysXXX for tab-specific updates");
    }

    let tty_path = Path::new(terminal_id);
    let mut file = OpenOptions::new()
        .write(true)
        .open(tty_path)
        .with_context(|| format!("failed to open tty {}", tty_path.display()))?;

    // OSC 11 = set background color and OSC 10 = set foreground color.
    // We set both to preserve text contrast/readability.
    let fg = contrasting_foreground(color_hex);
    let payload = format!("\x1b]11;{color_hex}\x07\x1b]10;{fg}\x07");
    file.write_all(payload.as_bytes())
        .with_context(|| format!("failed to write OSC sequence to {}", tty_path.display()))?;

    Ok(())
}

pub fn reset_dynamic_colors_for_tty(terminal_id: &str) -> Result<()> {
    if !terminal_id.starts_with("/dev/") {
        anyhow::bail!("terminal_id must be a tty path like /dev/ttysXXX for tab-specific updates");
    }

    let tty_path = Path::new(terminal_id);
    let mut file = OpenOptions::new()
        .write(true)
        .open(tty_path)
        .with_context(|| format!("failed to open tty {}", tty_path.display()))?;

    // OSC 110 = reset foreground, OSC 111 = reset background, OSC 112 = reset cursor color.
    let payload = "\x1b]110\x07\x1b]111\x07\x1b]112\x07";
    file.write_all(payload.as_bytes()).with_context(|| {
        format!(
            "failed to write reset OSC sequence to {}",
            tty_path.display()
        )
    })?;

    Ok(())
}

fn contrasting_foreground(color_hex: &str) -> &'static str {
    if color_hex.len() != 7 || !color_hex.starts_with('#') {
        return "#f5f5f5";
    }

    let r = u8::from_str_radix(&color_hex[1..3], 16).unwrap_or(0) as f32 / 255.0;
    let g = u8::from_str_radix(&color_hex[3..5], 16).unwrap_or(0) as f32 / 255.0;
    let b = u8::from_str_radix(&color_hex[5..7], 16).unwrap_or(0) as f32 / 255.0;

    // Relative luminance approximation.
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if luminance > 0.45 {
        "#111111"
    } else {
        "#f5f5f5"
    }
}

pub fn doctor_check(terminal_id: Option<&str>) -> (bool, String) {
    let Some(terminal_id) = terminal_id else {
        return (
            false,
            "no terminal id provided; expected tty path from `tty`".to_string(),
        );
    };

    if !terminal_id.starts_with("/dev/") {
        return (
            false,
            format!(
                "terminal id `{terminal_id}` is not a tty path; configure shell hook to pass `$(tty)`"
            ),
        );
    }

    if !Path::new(terminal_id).exists() {
        warn!(terminal_id, "tty path does not exist");
        return (false, format!("tty path does not exist: {terminal_id}"));
    }

    (true, format!("tty path looks valid: {terminal_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_tty_terminal_id() {
        let err = apply_background_color_to_tty("abc", "#112233").unwrap_err();
        assert!(err.to_string().contains("tty path"));

        let err = reset_dynamic_colors_for_tty("abc").unwrap_err();
        assert!(err.to_string().contains("tty path"));
    }

    #[test]
    fn picks_light_text_for_dark_background() {
        assert_eq!(contrasting_foreground("#112233"), "#f5f5f5");
    }

    #[test]
    fn picks_dark_text_for_light_background() {
        assert_eq!(contrasting_foreground("#d0d0d0"), "#111111");
    }
}
