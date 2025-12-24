# FileLens — Agent Guide

## Project overview
FileLens is a desktop metadata analyzer/cleaner built with **Tauri v2** (Rust backend) and a **React + Vite** frontend. The app inspects file metadata (including images and Office files), can remove metadata, and supports directory/file batch cleanup.

## Repository layout
- `src-tauri/` — Tauri (Rust) app configuration and build entrypoint.
- `src/` — Rust app logic and commands invoked from the frontend.
- `frontend/` — React + Vite UI.
- `tests/` — Rust tests (if present).

## Backend commands & events
Frontend calls Tauri commands via `@tauri-apps/api/core` `invoke`:
- `analyze_file(path, include_hash)`
- `analyze_directory(path, recursive)`
- `analyze_files(paths)`
- `remove_metadata(path)`
- `edit_office_metadata(path, field, value)`
- `start_cleanup(path, recursive, filter)`
- `start_cleanup_files(paths, filter)`
- `pick_file()`, `pick_directory()`, `pick_files()`
- `search_files(query)` and `search_directories(query)` (available but optional)

Cleanup progress is emitted as `cleanup://progress` with payloads:
`started`, `processing`, `success`, `failure`, `finished` (see `src-tauri/src/main.rs`).

## Logging requirements
Any new functionality must log warnings and errors to the Logs view with full error detail; avoid info/success logging to keep the log signal high. Use the centralized logger in `frontend/src/App.tsx` (the `logEvent` helper) and pass it down when a view/component needs to report warnings/errors. Implementation guidance:
- Only log `warning` and `error` levels; do not log `info` or `success`.
- Always include full error detail (pass the raw `Error` object or full payload so the logger can capture stack/message without truncation).
- Provide a short, actionable message and a stable context tag (e.g. `cleanup`, `analyze`, `export`, `office`, `picker`).
- Log only when something is wrong or blocked (validation failures, failed invokes, failed subscriptions, unexpected states); do not log routine events like drag & drop or normal progress.
- If a user-facing toast is shown for a warning/error, mirror it with a log entry at the same level.

## Drag & drop notes (Tauri v2)
File drops are handled via Tauri’s `onDragDropEvent` from the current webview/window.  
HTML5 drag-and-drop may not yield filesystem paths in the browser; the app relies on Tauri events for real file paths.

## Frontend structure (Atomic design)
The frontend uses an atomic-style component structure:
- `frontend/src/components/atoms/` — Small primitives (e.g. Button, Toggle).
- `frontend/src/components/molecules/` — Combined UI pieces (e.g. DropZone, SegmentedControl).
- `frontend/src/components/organisms/` — Larger UI blocks (Sidebar, Topbar, Toast).
- `frontend/src/components/layout/` — Layout wrappers (AppShell, Sheet, Section).
- `frontend/src/views/` — Screen-level views (AnalyzeView, CleanView).
- `frontend/src/utils/` — Shared helpers.
- `frontend/src/types/` — Shared types.
- `frontend/src/styles.css` — Global tokens/resets/utilities only.

Each component or view lives in its own folder with this structure:
```
<component-name>/
  Component.tsx
  Component.css
```

## UI entry points
- `frontend/src/App.tsx` orchestrates state and backend calls.
- `frontend/src/views/AnalyzeView/AnalyzeView.tsx` and `frontend/src/views/CleanView/CleanView.tsx` render the main screens.

## Icons & branding
App icons live under `src-tauri/icons/` and are referenced in `src-tauri/tauri.conf.json` under `bundle.icon`.
Replace those files to change the application icon across platforms.

## Running the app
- Dev (Tauri): `cargo tauri dev`
- Frontend only: `npm --prefix frontend run dev`

## Lint / checks (manual)
There are no preconfigured lint scripts, so use these checks when needed:
- **TypeScript type check:**
  ```
  npx --prefix frontend tsc -p frontend/tsconfig.json --noEmit
  ```
- **Rust formatting check:**
  ```
  cargo fmt -- --check
  ```
- **Rust lints (clippy):**
  ```
  cargo clippy --all-targets --all-features -D warnings
  ```

## Git commands
Do **not** run Git commands (e.g., `git status`, `git diff`, `git commit`, etc.) unless the user explicitly asks or grants permission.
