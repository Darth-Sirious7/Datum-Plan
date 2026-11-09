# Security

## Reporting a problem

Please report a security problem privately: use **Security › Report a vulnerability** on this repository (GitHub private advisory), or open an issue that only says you have something to report and we will contact you. Do not post details in a public issue.

## What Datum Plan does and does not do

- Plans are `.dplan` files (JSON) on your own computer. They can hold people's names, rates and notes; keep them where you keep other project documents.
- The only network call is the update check and download from this repository's GitHub Releases (HTTPS). An update installs only if the release has `SHA256SUMS.txt` and the installer matches its line exactly; the download goes to a new private folder, has a size limit, and is deleted if anything does not match.
- The window runs under a strict content security policy (no remote scripts, frames, objects or form posts) with only Tauri's core permissions; it can open only links to this repository (`open_url` refuses anything else).
- Imported and opened files (`.dplan`, XML, XER, workbooks (.xlsx), CSV, JSON) are treated as untrusted: they are size-limited (300 MB), out-of-range values are clamped when the plan is loaded (durations, lags and delays up to about 80 years; no non-finite numbers), and the engine is tested with thousands of damaged files, random plans, random edits and deliberately hostile values. None may crash or hang the app.
- Saves are atomic: the new file is written next to the old one and then replaces it, so a crash or a full disk never leaves a half-written plan. Autosave and recovery work the same way.
- CSV exports put an apostrophe before text that a spreadsheet would run as a formula (starting with `=`, `+`, `-` or `@`), so opening an exported file never runs anything. Workbook exports write task names as text.
- If anything inside the app fails, that action is stopped with a message and the plan stays as it was; the details go to `crash.log` in the app's data folder (nothing is sent anywhere).
- `datum-cli` and its MCP server (`datum-cli mcp`) run on your computer with your permissions. They change a plan only through the named operations, write only plan and export file types (by extension), and save only when asked.
- The installer is not code-signed yet (planned for 0.6), so Windows shows a SmartScreen warning on the first install.
