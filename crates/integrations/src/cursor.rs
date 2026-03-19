use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

const MARKER_KEY: &str = "worktreeSync.color";

pub fn apply_cursor_workspace_color(worktree_path: &Path, color: &str) -> Result<()> {
    let settings_path = worktree_path.join(".vscode/settings.json");
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut settings = if settings_path.exists() {
        let raw = std::fs::read_to_string(&settings_path)
            .with_context(|| format!("failed to read {}", settings_path.display()))?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !settings.is_object() {
        settings = json!({});
    }

    let obj = settings
        .as_object_mut()
        .expect("settings should be object after normalization");

    let mut customizations = obj
        .get("workbench.colorCustomizations")
        .cloned()
        .unwrap_or_else(|| json!({}));

    if !customizations.is_object() {
        customizations = json!({});
    }

    let custom_obj = customizations
        .as_object_mut()
        .expect("customizations should be object after normalization");

    custom_obj.insert("titleBar.activeBackground".to_string(), json!(color));
    custom_obj.insert(
        "titleBar.inactiveBackground".to_string(),
        json!(dimmed(color)),
    );
    custom_obj.insert("titleBar.activeForeground".to_string(), json!("#ffffff"));

    obj.insert(
        "workbench.colorCustomizations".to_string(),
        Value::Object(custom_obj.clone()),
    );

    // Keep Cursor integrated terminals anchored to the workspace root.
    obj.insert(
        "terminal.integrated.cwd".to_string(),
        json!("${workspaceFolder}"),
    );
    // New splits should start where the terminal started (workspace root), not the current shell cwd.
    obj.insert("terminal.integrated.splitCwd".to_string(), json!("initial"));

    obj.insert(MARKER_KEY.to_string(), json!(color));

    let out = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, out)
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
