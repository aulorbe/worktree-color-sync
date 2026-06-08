# Worktree Color Sync Extension

Watches `.vscode/settings.json` for color changes from worktree-sync and applies them dynamically to the title bar without requiring a window reload. Works with both VSCode and Devin IDE.

## Installation (Local/Unpacked)

### VSCode

1. Compile the extension:
   ```bash
   npm install
   npm run compile
   ```

2. Add to VSCode's extensions directory:
   ```bash
   mkdir -p ~/.vscode/extensions
   cp -r . ~/.vscode/extensions/worktree-color-sync-0.1.0
   ```

3. Restart VSCode

### Devin IDE

1. Compile the extension:
   ```bash
   npm install
   npm run compile
   ```

2. Add to Devin IDE's extensions directory:
   ```bash
   mkdir -p ~/Library/Application\ Support/Devin/extensions
   cp -r . ~/Library/Application\ Support/Devin/extensions/worktree-color-sync-0.1.0
   ```

3. Restart Devin IDE

## How it works

- Watches `.vscode/settings.json` in the workspace root
- When `worktreeSync.color` key is updated, reads the color
- Updates the `workbench.colorCustomizations` settings in real-time
- Title bar color changes instantly without reload
