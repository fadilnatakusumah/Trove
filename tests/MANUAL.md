# Manual / UI smoke test record (plan 08 §8.4, tech-plan §30)

## Why this is manual, not automated

Plan 08 §8.4 called for executing the §30 critical flow with the
`webapp-testing` agent skill (Playwright). **That is not applicable to this
app, and the plan should be corrected on this point.**

Agent Manager is a Tauri desktop app. Every screen gets its data through
`invokeCommand()` → Tauri IPC, which only exists inside the Tauri webview. A
plain browser pointed at the Vite dev server (`http://localhost:5173`) has no
`window.__TAURI_INTERNALS__`, so `detect_providers`, `get_skills`, and every
other command reject immediately. A Playwright run against the dev server would
render the error states and prove nothing about the real app.

The honest options are:

1. Run the built desktop app and verify by hand (what the record below is for).
2. Drive the **desktop** window with a WebDriver — `tauri-driver` + WebdriverIO.
   That is a real capability but it is new tooling, not something plan 08
   budgeted, and it only covers Windows/Linux (no macOS support).

This file records option 1. The service-layer equivalent of the same flow
**is** automated — see `src-tauri/tests/lifecycle_test.rs`
(`full_lifecycle_install_list_toggle_uninstall`), which covers install → list →
disable → enable → uninstall against fixture trees on every `cargo test`.

## How to run the smoke test

```bash
cd Trove
npm run tauri dev          # dev build, hot reload
# or, against a packaged build:
npm run tauri build && ./src-tauri/target/release/agent-manager
```

Then walk the table below. Record the result, the OS, and the date.

## Results

| # | Step | Expected | Result | Notes |
|---|------|----------|--------|-------|
| 1 | Launch the app | Window opens, Dashboard renders, no white flash | ⬜ | |
| 2 | Dashboard → Agents count | Matches installed agents; no crash on 0 | ⬜ | |
| 3 | Sidebar → Providers | Each supported agent shows Detected / Not found honestly | ⬜ | |
| 4 | Sidebar → Projects → Add Project | Native folder picker opens; project appears | ⬜ | |
| 5 | Add a second project, switch between them | Both listed, skills scoped correctly | ⬜ | |
| 6 | Sidebar → Skills | Global + project skills listed with provider/scope badges | ⬜ | |
| 7 | Search box | Filters instantly as you type | ⬜ | |
| 8 | Provider / scope / status filters | Each narrows the list; "disabled" shows disabled skills | ⬜ | |
| 9 | Click a skill row | Detail view opens with path, files, metadata | ⬜ | |
| 10 | Detail → Open Folder | OS file manager reveals the skill directory | ⬜ | |
| 11 | Detail → Disable | Skill flips to disabled; still listed; file renamed | ⬜ | |
| 12 | Detail → Enable | Skill returns to enabled | ⬜ | |
| 13 | Skills → Install Skill (Git URL) | Inspect lists the skill(s); Install copies them | ⬜ | |
| 14 | Install with an invalid repo | Actionable error (git's own reason), not a generic failure | ⬜ | |
| 15 | Install onto an occupied name | Conflict explains itself; offers "Move aside and install" | ⬜ | |
| 16 | Install to a project scope | Skill lands in the project's agent dir, not global | ⬜ | |
| 17 | Row → uninstall icon → confirm | Skill moves to OS trash; disappears from the list | ⬜ | |
| 18 | Confirm dialog → "Delete permanently" | Skill is deleted outright, not trashed | ⬜ | |
| 19 | Settings → Open Logs | OS file manager opens the log directory | ⬜ | |
| 20 | Settings → Git row | Shows the git version, or an actionable "not found" | ⬜ | |
| 21 | Keyboard: Tab through sidebar → list → dialog | Every control reachable; focus ring always visible | ⬜ | |
| 22 | Keyboard: ⌘K / Ctrl+K palette | Opens, filters, navigates, Escape closes | ⬜ | |
| 23 | Resize window to minimum (800×560) | Nothing overflows or is clipped; rows stack | ⬜ | |
| 24 | Close and relaunch | State persists (projects registry), no errors in the log | ⬜ | |

## Cross-platform matrix (§32)

Each OS needs its own pass of the table above, plus the platform-specific checks
below. Only the rows marked "verified" have actually been run.

| Platform | Bundle | Table pass | Path handling | Trash | Open Folder | Logs location |
|---|---|---|---|---|---|---|
| Windows 11 | msi / nsis | ⬜ | ⬜ | ⬜ | ⬜ | `%LOCALAPPDATA%\com.trove.agent-manager\logs` |
| macOS (Apple Silicon) | dmg | ⬜ | ⬜ | ⬜ | ⬜ | `~/Library/Logs/com.trove.agent-manager` |
| macOS (Intel) | dmg | ⬜ | ⬜ | ⬜ | ⬜ | same as above |
| Linux (Ubuntu) | deb / AppImage | ⬜ | ⬜ | ⬜ | ⬜ | `~/.local/share/com.trove.agent-manager/logs` |

Bundle builds confirmed so far (build only — the table above still needs a
manual pass on each OS):

| Platform | Result |
|---|---|
| Windows x64 | ✅ `Agent Manager_0.1.0_x64_en-US.msi` (3.61 MiB) + `Agent Manager_0.1.0_x64-setup.exe` (2.44 MiB), release profile, 0 warnings |
| macOS | ⬜ not built — no macOS runner available |
| Linux | ⬜ not built — no Linux runner available |

Platform-specific things worth checking explicitly, because they are the ones
that actually differ:

- **Home resolution** — `~/.claude/skills` resolves correctly (Windows uses
  `%USERPROFILE%`; the app normalizes separators for display).
- **Trash** — uninstall uses the OS recycle bin / Trash, and the item is
  genuinely restorable.
- **Log directory** — matches the column above; the file exists after a run.
- **No hardcoded Unix paths** — `grep -rnE '"/tmp|"/Users|"/home|"/etc' src/`
  returns only test fixtures.
