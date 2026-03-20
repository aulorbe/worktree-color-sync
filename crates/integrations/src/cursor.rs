use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

const MARKER_KEY: &str = "worktreeSync.color";

pub fn apply_cursor_workspace_color(worktree_path: &Path, color: &str) -> Result<()> {
    let settings_path = worktree_path.join(".vscode/settings.json");
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    // Create empty settings file if it doesn't exist
    if !settings_path.exists() {
        std::fs::write(&settings_path, "{}\n")?;
    }

    let dimmed_color = dimmed(color);

    // Use jq to surgically update only the fields we need, preserving everything else
    let jq_filter = format!(
        ". + {{\"workbench.colorCustomizations\": (.\"workbench.colorCustomizations\" // {{}}) + \
         {{\"titleBar.activeBackground\": \"{}\", \"titleBar.inactiveBackground\": \"{}\", \
         \"titleBar.activeForeground\": \"#ffffff\"}}, \
         \"terminal.integrated.cwd\": \"${{workspaceFolder}}\", \
         \"terminal.integrated.splitCwd\": \"initial\", \
         \"{}\": \"{}\"}}",
        color, dimmed_color, MARKER_KEY, color
    );

    let output = Command::new("jq")
        .arg(&jq_filter)
        .arg(&settings_path)
        .output()
        .context("failed to run jq - is it installed?")?;

    if !output.status.success() {
        anyhow::bail!(
            "jq failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    std::fs::write(&settings_path, output.stdout)
        .with_context(|| format!("failed to write {}", settings_path.display()))?;

    Ok(())
}

fn dimmed(color: &str) -> String {
    if color.len() != 7 || !color.starts_with('#') {
        return color.to_string();
    }

    let r = u8::from_str_radix(&color[1..3], 16).unwrap_or(0);
    let g = u8::from_str_radix(&color[3..5], 16).unwrap_or(0);
    let b = u8::from_str_radix(&color[5..7], 16).unwrap_or(0);

    format!("#{:02x}{:02x}{:02x}", r / 2, g / 2, b / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use tempfile::tempdir;

    #[test]
    fn preserves_unrelated_keys() {
        let dir = tempdir().unwrap();
        let vscode = dir.path().join(".vscode");
        std::fs::create_dir_all(&vscode).unwrap();

        let settings_path = vscode.join("settings.json");
        std::fs::write(
            &settings_path,
            r##"{"editor.fontSize":14,"workbench.colorCustomizations":{"terminal.foreground":"#ff00ff"}}"##,
        )
        .unwrap();

        apply_cursor_workspace_color(dir.path(), "#112233").unwrap();

        let raw = std::fs::read_to_string(settings_path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(value["editor.fontSize"], json!(14));
        assert_eq!(
            value["workbench.colorCustomizations"]["terminal.foreground"],
            json!("#ff00ff")
        );
        assert_eq!(
            value["workbench.colorCustomizations"]["titleBar.activeBackground"],
            json!("#112233")
        );
        assert_eq!(
            value["terminal.integrated.cwd"],
            json!("${workspaceFolder}")
        );
        assert_eq!(value["terminal.integrated.splitCwd"], json!("initial"));
    }

    #[test]
    fn cycle_color_updates_existing_color() {
        let dir = tempdir().unwrap();

        // Apply initial color
        apply_cursor_workspace_color(dir.path(), "#111111").unwrap();

        let settings_path = dir.path().join(".vscode/settings.json");
        let raw = std::fs::read_to_string(&settings_path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            value["workbench.colorCustomizations"]["titleBar.activeBackground"],
            json!("#111111")
        );

        // Cycle to new color
        apply_cursor_workspace_color(dir.path(), "#999999").unwrap();

        let raw = std::fs::read_to_string(&settings_path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            value["workbench.colorCustomizations"]["titleBar.activeBackground"],
            json!("#999999")
        );
        assert_eq!(
            value["worktreeSync.color"],
            json!("#999999")
        );
    }

    #[test]
    fn applies_color_to_empty_worktree() {
        let dir = tempdir().unwrap();

        // Apply color to a worktree that has never had settings before
        apply_cursor_workspace_color(dir.path(), "#abcdef").unwrap();

        let settings_path = dir.path().join(".vscode/settings.json");
        assert!(settings_path.exists(), "settings.json should be created");

        let raw = std::fs::read_to_string(&settings_path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(
            value["workbench.colorCustomizations"]["titleBar.activeBackground"],
            json!("#abcdef")
        );
        assert_eq!(
            value["worktreeSync.color"],
            json!("#abcdef")
        );
        assert_eq!(
            value["terminal.integrated.cwd"],
            json!("${workspaceFolder}")
        );
    }
}
