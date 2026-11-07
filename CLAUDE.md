# Datum Plan: notes for Claude

Read `docs/MEMORY.md` first. It is the project's memory: architecture, decisions, roadmap, and a log of every session.
At the end of every session that changes anything, update `docs/MEMORY.md`:
- tick the roadmap
- add a session-log entry
- record any new decisions
- refresh the architecture section if files moved

User preferences:
- Rust and Tauri 2, small and efficient code, Windows first.
- Ship in small numbered releases (0.1.0, 0.1.1, …). Every release appears in the in-app updater.
- A light theme with blue accents (since 0.1.1), a ribbon layout with the task sheet and Gantt chart side by side, advanced scheduling features (activity codes, steps, longest path, earned value, risk), and an icon sidebar for the views.
- Keep code compact and match the surrounding style. Engine logic belongs in `crates/core` (pure Rust, unit-tested), not in the UI.
- The UI is SolidJS + Vite in `ui/src/*.tsx` (built to `ui/dist`). State lives in `ui/src/store.ts`; the engine computes all values, the window only shows them.

Checks before pushing:
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm run check:ui`, then `npm run build:ui` and the UI tests (`node tests_ui/run.cjs`)
- For changes to the grid, the Gantt or what the engine sends per edit: `npm run perf:ui` (the window with 100,000 rows) and `npm run bench` (engine times, edit cycle and memory at 100,000 tasks)
- `cargo deny check` (licences, advisories, sources; CI runs it)

Releasing:
1. Bump the version in `src-tauri/Cargo.toml`, `crates/core/Cargo.toml` and `crates/cli/Cargo.toml`.
2. Add a CHANGELOG section, and keep `docs/site.json` true to the release (`about`, `steps`, `highlights`, `roadmap`; no commercial product names).
3. If the release changes how something is used, update the guide (`docs/guide/en.md` and `zh.md`, both languages), then `npm run build:ui && npm run guide` (screenshots and PDFs; needs Playwright and `cargo run -p datum-core --example dump guide > tests_ui/guide_fixture.json` and `... dump tutorial > tests_ui/tutorial_fixture.json`). The release attaches `docs/guide/pdf/*.pdf`.
4. Push the tag `vX.Y.Z`, or run `release.yml` with workflow_dispatch when tag pushes are blocked. The workflow creates the tag and publishes the installer.
