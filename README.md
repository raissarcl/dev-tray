# Dev Tray

Lightweight **Windows** system-tray launcher for local development projects.

Keeps Vite/Next/Node/Docker Compose (any command) running without dedicated terminal windows. Start, stop, restart, open browser/Cursor/folder, and inspect recent logs from the tray.

> Windows only — process trees use Windows Job Objects.

## Features

- Resident **system tray** menu (no window required at boot)
- Process control with Windows **Job Objects** (kills full process trees)
- Event-driven status updates (no polling)
- JSON config kept with the app (`projects.json` in the project root)
- Optional dashboard window (created on demand, destroyed on close)

## Requirements

- Windows 10/11
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (stable toolchain)
- Cursor or VS Code on `PATH` (optional — for **Open editor**)

## Quick start

1. Copy the example config and edit paths/commands:

   ```bash
   copy projects.example.json projects.json
   ```

2. Install and run in development:

   ```bash
   npm install
   npm run tauri:dev
   ```

3. For day-to-day use (no console window), build once then start detached:

   ```bash
   start-dev-tray.bat
   ```

   That builds if needed and launches `src-tauri\target\release\tray-for-projects.exe` in the background. Look for the tray icon — including the `^` overflow area if Windows hid it.

To start with Windows: create a shortcut to that `.exe` in `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`.

`tauri:dev` is for development only. Prefer the release `.exe` / `start-dev-tray.bat` for normal use.

## Config

Edit **`projects.json` in this repo’s root** (same folder as `README.md` / `start-dev-tray.bat`).

- Committed template: [`projects.example.json`](projects.example.json)
- Your real list: `projects.json` (gitignored — personal paths stay local)

On first run, if `projects.json` is missing, it is created by copying `projects.example.json` into the project root.

Resolution order:

1. `DEV_TRAY_CONFIG` env var (optional override)
2. `./projects.json` (working directory)
3. `projects.json` next to the `.exe`, or walking up to the app/repo root
4. Legacy fallback: `%APPDATA%\tray-for-projects\projects.json`

Example entry:

```json
{
  "version": 1,
  "settings": {
    "editor": "cursor",
    "log_lines": 500
  },
  "projects": [
    {
      "id": "my-app",
      "name": "My App",
      "path": "D:\\Projects\\my-app",
      "command": "npm run dev",
      "port": 5173,
      "url": "http://localhost:5173",
      "icon": "app"
    }
  ]
}
```

After editing, use **Reload Config** in the tray menu (or restart Dev Tray).

## Build

```bash
npm run tauri:build
```

Or manually:

```bash
npm run build
cd src-tauri && cargo build --release
```

## Architecture

Rust owns the tray, config, and processes. The TypeScript UI is optional and talks via Tauri commands/events (`project://status`).

```text
src-tauri/src/
  domain/      models (Project, Settings, status)
  config/      load / reload projects.json
  process/     spawn, Job Objects, log buffer, events
  actions/     open browser / editor / folder
  tray/        tray menu + handlers
  window/      optional dashboard lifecycle
  commands.rs  Tauri IPC for the dashboard
  app.rs       composition / bootstrap

src/
  main.ts      dashboard boot + event wiring
  api.ts       Tauri command clients
  ui.ts        DOM rendering helpers
  types.ts     shared view types
```

## License

MIT — see [LICENSE](LICENSE).
