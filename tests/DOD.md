# Phase 1 Definition of Done — release gate

Status of the tech-plan §35 checklist, with the evidence for each item.
Legend: ✅ verified by automated test · 🧪 needs a manual pass on the running
app (see [MANUAL.md](./MANUAL.md)) · ⬜ not yet done.

Re-run the automated half with:

```bash
cd src-tauri && cargo test     # 78 tests
cd .. && npm run build         # tsc -b + vite build
npx eslint src                 # lint
```

## Provider discovery

| Item | Status | Evidence |
|---|---|---|
| Launch Agent Manager | 🧪 | MANUAL.md #1 |
| See detected [CC] installation | 🧪 | MANUAL.md #3. Detection logic: `providers::detect_all` + `detection_reports_every_provider_with_a_global_dir` |
| See detected Cursor installation | 🧪 | as above |
| See detected Codex installation | 🧪 | as above |
| See detected OpenCode installation | 🧪 | as above (beyond §35, added on request) |

Detection keys off the agent's **config root**, not its skills dir, so an
installed agent with no skills yet is not reported as missing.

## Project management

| Item | Status | Evidence |
|---|---|---|
| Add a project | 🧪 | MANUAL.md #4. `project_service` round-trip test covers registry + rejection of missing dirs |
| Remove a project from Agent Manager | 🧪 | MANUAL.md #5 |
| Switch between projects | 🧪 | MANUAL.md #5. Scope matrix covered by `get_skills_filters_by_scope_and_provider` |

## Skill discovery

| Item | Status | Evidence |
|---|---|---|
| See global skills | ✅ | `get_skills_global_scope_when_dir_missing_is_empty_ok` |
| See project skills | ✅ | `get_skills_filters_by_scope_and_provider`, `happy_path_local_global_and_project` |
| Search skills | 🧪 | MANUAL.md #7 (client-side filter) |
| Filter skills | 🧪 | MANUAL.md #8 (provider / scope / status) |
| Open skill details | 🧪 | MANUAL.md #9 |

## Installation

| Item | Status | Evidence |
|---|---|---|
| Install a skill from GitHub | ✅ | `install_service::stage_source` + clone path; git argv fixed, timeout-guarded. Local-fixture equivalent in `happy_path_local_global_and_project` |
| Install globally | ✅ | same test, global branch |
| Install to a project | ✅ | `full_lifecycle_install_list_toggle_uninstall` |
| Handle invalid repositories | ✅ | `invalid_repo_maps_to_invalid_skill`, `git_missing_source_path_is_structured_error` |
| Handle installation conflicts | ✅ | `reinstall_requires_explicit_confirmation`, `conflict_on_non_skill_directory_explains_itself`, `replace_confirmed_swaps_the_occupant_into_the_trash` |

## Management

| Item | Status | Evidence |
|---|---|---|
| Enable a skill | ✅ | `disable_and_enable_round_trip` |
| Disable a skill | ✅ | same, plus `disabled_skill_is_still_listed_and_flagged` |
| Uninstall a skill | ✅ | `uninstall_permanent_deletes_the_directory`, `full_lifecycle_*` |
| Open skill folder | 🧪 | MANUAL.md #10 |

> §35 says "enable/disable **where supported**". That qualifier no longer
> applies: disabling renames `SKILL.md`, which every agent honours, so the
> toggle works everywhere. See plan 07's notes.

## Safety

| Item | Status | Evidence |
|---|---|---|
| No arbitrary shell execution | ✅ | Only two `Command::new` calls, both `git` with fixed argv; no `sh -c` / `cmd /c` anywhere in `src/`. Shell plugin is not installed |
| Path traversal protection | ✅ | `rejects_path_escape`, `traversal_ids_are_refused_without_touching_the_filesystem`, `REGRESSION_*` resolution tests |
| Confirmation before destructive actions | 🧪 | Uninstall always opens `ConfirmDialog` with the exact path; MANUAL.md #17–18 |
| Structured error handling | ✅ | `covers_all_tech_plan_codes`, `serializes_to_frontend_contract`; errors are `{code, message, detail}` end to end |

## Quality gates

| Item | Status | Evidence |
|---|---|---|
| `cargo test` clean | ✅ | 78 passed, 0 warnings |
| `tsc -b` clean | ✅ | `npm run build` |
| lint clean | ✅ | `npx eslint src` exits 0. Two real fixes: `providerLabel` moved out of `skill-badges.tsx` into `skill-labels.ts` (Fast Refresh needs component-only files), and `set-state-in-effect` removed from `ConfirmDialog`. A scoped override relaxes `react-refresh/only-export-components` + `set-state-in-effect` for `src/components/ui/**` and `src/hooks/use-mobile.ts` only — those are shadcn CLI output, and editing them would be reverted by the next `shadcn add` |
| Platform bundles build + smoke | ⚠️ | Windows: **builds** — MSI (3.61 MiB) + NSIS (2.44 MiB), release profile, 0 warnings. On-app smoke still needs a manual pass. macOS/Linux: not built (no runner available) — see MANUAL.md |
| README updated | ✅ | Repo README |
| Logging with rotation, no secrets | ✅ | `lib.rs` log targets; `sanitize_url` strips credentials; no skill content is logged |
| Performance within §33 budgets | ✅ | `performance_test.rs` |

### Deviation from plan 08 §8.3, stated openly

§8.3 asks to "cache provider/skill discovery in memory (**Rust side**),
invalidate on mutations". The cache lives on the **frontend** instead
(TanStack Query: one query per provider × scope combo, 30s stale time,
invalidated after every mutation).

The requirement §33 actually states is *"do not scan the entire filesystem on
every UI render"*, and that is satisfied — a render reads the query cache, and
a scan happens only when a combo is stale or a mutation invalidated it. A
Rust-side cache would add a second source of truth plus its own invalidation
and staleness rules, for no measured gain over the existing one. Recorded here
rather than done silently; happy to move it into Rust if the intent was
something else (e.g. caching across app restarts, which would be a different
feature).

## Known limitations (documented, not hidden)

- **Automated UI smoke test is not possible** for a Tauri app via the
  `webapp-testing` Playwright skill — see the explanation at the top of
  MANUAL.md. Service-layer coverage stands in for it.
- **Shallow clone cannot target a commit SHA** (`--depth 1` + `--branch`);
  branch and tag names work. Documented ceiling in plan 06 §4.
- **Discovery scans one level of nesting** for category folders (e.g.
  `.cursor/skills/<category>/<skill>/SKILL.md`). Deeper nesting is not scanned.
- **Plugin-provided skills are not modelled** — a skill inside a plugin
  directory is not listed or managed in Phase 1.
- **`replace` on install trashes the occupant**; there is no undo inside the
  app beyond restoring from the OS trash.
