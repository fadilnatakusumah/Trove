# Agent Manager

A desktop app for managing AI coding-agent **skills** — the `SKILL.md` folders
that [CC], Cursor, Codex, and OpenCode load. See which skills are installed
where, install new ones from Git or a local folder, and enable, disable, or
remove them — without hand-editing hidden directories.

Built with **Tauri 2** (Rust) + **React 19 / TypeScript / Vite / Tailwind /
shadcn/ui**.

## What it does

- **Discovers** your installed agents and every skill they would load, in both
  global (`~/.claude/skills`) and project (`.claude/skills`) scope.
- **Installs** skills from a Git URL (including pasted GitHub `blob`/`tree`
  links) or a local folder, after showing you exactly what it found.
- **Toggles** a skill on or off by renaming its `SKILL.md` — lossless and
  reversible, and it works for every agent because they all discover skills by
  that filename.
- **Removes** a skill to the OS trash (restorable), or permanently on explicit
  confirmation.
- **Explains** failures. Errors are structured (`code` + human copy + detail),
  so you get git's own reason for a bad clone instead of "installation failed".

## Requirements

- **Node.js** 20+ and **npm**
- **Rust** 1.90+ (edition 2024)
- **Git** — required only to install from a Git URL; local folders work
  without it. Settings → Diagnostics tells you whether git was found.
- Platform build tooling for Tauri: see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## Build and run

```bash
npm install

# Development (hot reload; starts Vite on :5173)
npm run tauri dev

# Packaged build for the current OS
npm run tauri build
```

Bundles land in `src-tauri/target/release/bundle/`.

## Test and check

```bash
# Rust: unit + integration + performance suites (78 tests)
cd src-tauri && cargo test

# Frontend: type-check + production build
npm run build

# Lint
npx eslint src
```

The Rust suite covers the safety-critical paths — path-traversal guards,
zero-overwrite installs, the full install → disable → enable → uninstall
lifecycle, and §33 performance budgets. See
[`tests/DOD.md`](./tests/DOD.md) for the release checklist and
[`tests/MANUAL.md`](./tests/MANUAL.md) for the on-app smoke test.

## Logs

The app writes a rotating log file (capped at ~1 MB) on every run, including
packaged builds. Open it from **Settings → Open Logs**.

| Platform | Location |
|---|---|
| Windows | `%LOCALAPPDATA%\com.trove.agent-manager\logs` |
| macOS | `~/Library/Logs/com.trove.agent-manager` |
| Linux | `~/.local/share/com.trove.agent-manager/logs` |

Credentials are stripped from Git URLs before they reach a log line, and skill
file contents are never logged.

## How it is put together

```
src-tauri/src/
  models/            Domain types — the single source of truth for the TS contract
  errors/            Structured AppError { code, message, detail }
  providers/         One module per agent: where its skills live, how to parse them
  services/
    filesystem_service.rs   The ONLY module that mutates the filesystem
    install_service.rs      Stage → inspect → validate → install
    lifecycle_service.rs    Enable/disable (rename) and uninstall
  commands/          Thin Tauri IPC entry points
src/
  features/          One folder per screen; queries + mutations per feature
  components/ui/     shadcn/ui primitives
```

Two invariants worth knowing before changing anything:

1. **The filesystem is the source of truth.** No shadow state in the frontend;
   mutations invalidate queries rather than editing the cache.
2. **`providers::is_skill_dir` is the one definition of "a skill."**
   Discovery, the lifecycle service, and the installer all call it, so they
   cannot disagree about what exists.

And one rule for anything the user reads:

3. **Never format a path with `.display()` or by hand.** Use
   `filesystem_service::display_path`, which shortens the home directory to `~`
   and normalizes separators. A raw `.display()` on Windows produces
   `C:\Users\me\.config/opencode/skills\frontend-design` — backslashes from the
   OS mixed with forward slashes from segments we joined ourselves. Paths are
   *compared* and *sent* in their real form; `display_path` is for showing
   only, and the frontend mirrors it with `formatPath()` from `src/lib/paths.ts`
   (it cannot shorten `~` itself, so the backend sends `displayPath` where the
   UI needs one).

Every filesystem mutation goes through `filesystem_service`, which guards its
target with `ensure_within` against an approved agent directory — a path or
skill id that tries to escape is rejected before anything is touched.

## Safety model

- No arbitrary shell execution. The only external process is `git`, invoked
  with fixed argv (never a shell string) and a timeout.
- Nothing from a cloned repository is ever executed. Symlinks are skipped
  during copy.
- Existing skills are never silently overwritten; replacing is an explicit,
  confirmed action.
- Deletions go to the OS trash unless you specifically ask for permanent.
- Skill files are treated as untrusted input: text reads are size-capped.

## License

Private / unpublished.
