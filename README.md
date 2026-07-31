# Dev Tray

Lightweight Windows system-tray launcher for local development projects.

Keeps Vite/Next/Node/Docker Compose (any command) running without dedicated terminal windows. Start, stop, restart, open browser/Cursor/folder, and inspect recent logs from the tray.

## Features (MVP)

- Resident **system tray** menu (no window required at boot)
- Process control with Windows **Job Objects** (kills full process trees)
- Event-driven status updates (no polling)
- JSON config in `%APPDATA%\tray-for-projects\projects.json`
- Optional dashboard window (created on demand, destroyed on close)

## Run without a terminal

Build once, then start detached (no console window):

```bash
npm run build
cd src-tauri && cargo build --release
```

Or simply double-click / run:

```bash
start-dev-tray.bat
```

That launches `src-tauri\target\release\tray-for-projects.exe` in the background. Look for the tray icon — including the `^` overflow area if Windows hid it.

To start with Windows: create a shortcut to that `.exe` in `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`.

## Develop (needs terminal)

```bash
npm install
npm run tauri:dev
```

`tauri:dev` is only for development. Day-to-day use should be the release `.exe` / `start-dev-tray.bat`.

## Config

On first run, `projects.example.json` is copied to the AppData config path (or a built-in default is written). Edit the config to point at your projects:

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

## Build

```bash
npm run tauri:build
```

## Architecture

Rust owns tray, config, and processes. TypeScript UI is optional and talks via Tauri commands/events (`project://status`).
