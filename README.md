# Agent Notifications (anot)

Desktop notifications for your coding agents. This project focuses on integrating with Claude Code via its hook system, showing you timely macOS/Linux notifications for events like tool usage, prompts, and session lifecycle.

Note: This is early-stage software. Expect rough edges.

## Features
- Claude Code hook integration (init helper to wire it up)
- Notifications for multiple events (pre/post tool use, notifications, prompts, start/stop, etc.)
- macOS and Linux desktop support (via `notify-rust`)

## Install
- With Cargo (local checkout): `cargo install --path .`
- Or build locally: `cargo build --release` and use `target/release/anot`

## Quick Start (Claude Code)
1) Run the initializer and follow prompts:
   - `anot init claude`
   - Pick where to write hooks: `~/.claude/settings.json`, `.claude/settings.json`, or `.claude/settings.local.json`.
   - Select which events should trigger notifications.

2) You’re done. Claude Code will invoke `anot claude` for the selected events to show notifications.

### What the initializer does
It edits your chosen Claude Code settings file and adds hook entries that execute the `anot claude` command on selected events. Re-running the initializer updates/removes prior `anot` hooks and applies your latest selection.

## Manual Configuration (optional)
If you prefer to edit your settings file directly, add entries like this:

```json
{
  "hooks": {
    "Notification": [
      { "hooks": [ { "type": "command", "command": "/absolute/path/to/anot claude", "timeout": 10 } ] }
    ],
    "PreToolUse": [
      { "matcher": "*", "hooks": [ { "type": "command", "command": "/absolute/path/to/anot claude", "timeout": 10 } ] }
    ]
  }
}
```

Notes:
- `Notification`, `UserPromptSubmit`, `Stop`, `SubagentStop`, `PreCompact`, `SessionStart`, `SessionEnd` don’t require a `matcher`.
- `PreToolUse` and `PostToolUse` support `matcher` (exact, regex, `*`, or empty string).

## CLI
- `anot` global options:
  - `--config <FILE>`, `-c <FILE>`: Path to `a-notifications.json` (default is under your system config dir, e.g., `~/.config/agent_notifications/a-notifications.json`).
  - `--reset-config`, `-r`: Recreate default config file if it exists.
  - `--debug`, `-d`: Increase debug level (repeatable).

- Commands:
  - `anot init claude [<path-to-settings.json>]`: Interactive setup for Claude Code hooks. If no path is provided, you’ll be prompted to choose.
  - `anot claude`: Processes a Claude Code hook event from stdin and emits JSON hook output. Used by the hooks you configure.

View help: `anot --help`, `anot init --help`

## Test Locally
You can simulate a Claude Code hook by piping JSON into `anot claude`:

```bash
echo '{
  "session_id": "abc123",
  "transcript_path": "/path/to/transcript.jsonl",
  "hook_event_name": "Notification",
  "message": "Hello from anot"
}' | anot claude
```

Expected:
- A desktop notification appears.
- The tool writes a JSON response to stdout (suppressed by Claude Code in normal operation).

## Configuration File
`anot` keeps its own config at:
- Default: `~/.config/agent_notifications/a-notifications.json` (created on first run)
- Override with `--config <FILE>`
- Reset with `--reset-config`

Currently, internal config is minimal and safe to ignore unless you want a custom path.

## Uninstall / Remove Hooks
- Run `anot init claude` and deselect all events to remove existing `anot` hooks from the chosen settings file.
- Or manually delete the relevant entries in your Claude Code settings.

## Screenshot
![macOS Notification](assets/macos-notification.png)

## Troubleshooting
- No notifications on Linux: ensure a desktop notification daemon is running (DBus notifications) and your environment supports it.
- No notifications on macOS: check Notification Center permissions for your terminal.
- Nothing happens: re-run `anot init claude` and confirm hooks are added to the expected settings file.
- Paths: the hook command must be an absolute path to `anot`.

## License
GPL-3.0-or-later. See `LICENSE`.
