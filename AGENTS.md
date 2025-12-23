# FileLens — Agent Guide

## Project overview
FileLens is a desktop metadata analyzer/cleaner built with **Tauri v2** (Rust backend) and a **React + Vite** frontend. The app inspects file metadata (including images and Office files), can remove metadata, and supports directory/file batch cleanup.

## Repository layout
- `src-tauri/` — Tauri (Rust) app configuration and build entrypoint.
- `src/` — Rust app logic and commands invoked from the frontend.
- `frontend/` — React + Vite UI.
- `tests/` — Rust tests (if present).

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
