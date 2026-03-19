use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn worktree_sync_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates
    path.pop(); // root
    path.push("target");
    path.push("debug");
    path.push("worktree-sync");
    path
}

#[test]
fn cycle_color_writes_cursor_settings_file() {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();

    // Initialize a git repo
    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()
        .expect("failed to init git repo");

    Command::new("git")
        .args(&["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("failed to set git user");

    Command::new("git")
        .args(&["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("failed to set git email");

    // Create a dummy file and commit so it's a valid repo
    std::fs::write(repo_path.join("test.txt"), "test").unwrap();
    Command::new("git")
        .args(&["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .expect("failed to git add");

    Command::new("git")
        .args(&["commit", "-m", "initial"])
        .current_dir(repo_path)
        .output()
        .expect("failed to git commit");

    // Start the daemon in the background
    let mut daemon = Command::new(worktree_sync_bin())
        .arg("daemon")
        .spawn()
        .expect("failed to start daemon");

    // Give daemon time to start
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Run cycle-color with explicit path
    let output = Command::new(worktree_sync_bin())
        .args(&["cycle-color", "--worktree-path", repo_path.to_str().unwrap()])
        .output()
        .expect("failed to run cycle-color");

    assert!(output.status.success(), "cycle-color command failed: {:?}", output);

    // Verify .vscode/settings.json was created
    let settings_path = repo_path.join(".vscode/settings.json");
    assert!(
        settings_path.exists(),
        ".vscode/settings.json should exist after cycle-color"
    );

    // Read and verify the file contains color customizations
    let contents = std::fs::read_to_string(&settings_path).expect("failed to read settings.json");
    let json: serde_json::Value = serde_json::from_str(&contents).expect("invalid json");

    assert!(
        json["workbench.colorCustomizations"]["titleBar.activeBackground"].is_string(),
        "titleBar.activeBackground should be set"
    );

    let color = json["workbench.colorCustomizations"]["titleBar.activeBackground"]
        .as_str()
        .unwrap();
    assert!(color.starts_with('#'), "color should be a hex code");
    assert_eq!(color.len(), 7, "color should be #RRGGBB format");

    // Run cycle-color again and verify the color changed
    let output2 = Command::new(worktree_sync_bin())
        .args(&["cycle-color", "--worktree-path", repo_path.to_str().unwrap()])
        .output()
        .expect("failed to run cycle-color second time");

    assert!(output2.status.success(), "second cycle-color command failed: {:?}", output2);

    // Clean up: kill the daemon
    let _ = daemon.kill();
    std::thread::sleep(std::time::Duration::from_millis(200));

    let contents2 = std::fs::read_to_string(&settings_path).expect("failed to read settings.json");
    let json2: serde_json::Value = serde_json::from_str(&contents2).expect("invalid json");

    let color2 = json2["workbench.colorCustomizations"]["titleBar.activeBackground"]
        .as_str()
        .unwrap();

    assert_ne!(
        color, color2,
        "color should have changed after second cycle-color"
    );
}
