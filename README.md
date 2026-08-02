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
- **Local aliases** via Windows `hosts` + reverse proxy (`http://app-a` → project port)

## Requirements

- Windows 10/11
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (stable toolchain)
- Cursor or VS Code on `PATH` (optional — for **Open editor**)
- Administrator once (optional) — needed to write `hosts` and bind port **80** for clean URLs

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

Example (two Vite apps on different ports — put the port in `command`; `port` drives the alias proxy):

```json
{
  "version": 1,
  "settings": {
    "editor": "cursor",
    "log_lines": 500,
    "proxy_enabled": true,
    "proxy_port": null,
    "manage_hosts": true,
    "hosts_persist": false
  },
  "projects": [
    {
      "id": "app-a",
      "name": "App A",
      "path": "D:\\Projects\\app-a",
      "command": "npm run dev -- --host 127.0.0.1 --port 5173",
      "port": 5173,
      "url": "http://localhost:5173",
      "icon": "app"
    },
    {
      "id": "app-b",
      "name": "App B",
      "path": "D:\\Projects\\app-b",
      "command": "npm run dev -- --host 127.0.0.1 --port 5174",
      "port": 5174,
      "url": "http://localhost:5174",
      "icon": "app"
    }
  ]
}
```

### Settings

| Field | Default | Notes |
|-------|---------|--------|
| `editor` | `"cursor"` | Editor binary for **Open Cursor** (`cursor` / `code`, or full path via `editor_path`). |
| `log_lines` | `500` | Ring-buffer size for **Show Logs**. |
| `editor_path` | `null` | Optional absolute path to the editor executable. |
| `proxy_enabled` | `true` | Local reverse proxy for project aliases. |
| `proxy_port` | `null` | Forced listen port. `null` tries **80**, then falls back to **8787**. |
| `manage_hosts` | `true` | Sync aliases into the Windows `hosts` file while running. |
| `hosts_persist` | `false` | Keep the hosts block after Quit. Default clears it. |

### Project fields

| Field | Required | Notes |
|-------|----------|--------|
| `id` | yes | **Must be unique**. Default hostname alias (`http://{id}`). |
| `alias` | no | Override hostname (default = `id`). Must be unique across projects. |
| `name` | yes | Label in the tray menu. |
| `path` | yes | Working directory for `command`. |
| `command` | yes | Spawned with `cmd /C` in `path`. |
| `port` | for aliases | Real listen port of the app; used by the reverse proxy. |
| `url` | fallback | Used if the proxy is off or `port` is missing. |
| `env` | no | Extra environment variables for the process. |
| `icon` / `kind` | no | Metadata only. |

- **Vite:** set host + port in `command` (`-- --host 127.0.0.1 --port 5173`). Default Vite bind is often IPv6-only on Windows, which breaks the alias proxy unless you pass `--host 127.0.0.1` (or `--host`). Keep the `port` field in sync.
- **Next.js / many Node apps:** you can also use `"env": { "PORT": "3001" }`.
- Duplicate `id` / hostname values are rejected when the config loads.

### Local aliases (`http://app-a`)

While Dev Tray is running:

1. It writes a managed block to `C:\Windows\System32\drivers\etc\hosts` (between `# BEGIN dev-tray` / `# END dev-tray`).
2. It listens on **`127.0.0.1:80`** and routes by `Host` header to each project’s `port` (HTTP + WebSocket for Vite HMR).

Then you can open:

- `http://app-a`
- `http://app-b`

**Open browser** in the tray uses that alias URL when the proxy is up.

On **Quit**, the hosts block is removed unless `"hosts_persist": true`.

If writing `hosts` or binding port 80 fails (no admin / port busy):

- Proxy falls back to **8787**
- URLs become `http://app-a:8787`, or `http://app-a.localhost` / `http://app-a.localhost:8787` when hosts could not be updated

Force a port with `"proxy_port": 8787`. Disable with `"proxy_enabled": false` or `"manage_hosts": false`.

If Vite HMR fails behind the proxy, set in that project’s `vite.config`:

```js
server: { hmr: { clientPort: 80 } } // or 8787 if using the fallback
```

After editing config, use **Reload Config** in the tray menu (or restart Dev Tray).

### Tray menu

- Each project is a submenu: Start / Stop / Restart / Open Browser / Open Cursor / Open Folder / Show Logs.
- Status prefix: 🟢 running (or starting), ⚪ stopped.
- **Reload Config** reloads `projects.json` and re-syncs hosts + proxy routes.
- **Quit** stops all projects, clears the hosts block (unless `hosts_persist`), and exits.

## Security notes

Dev Tray is a **local, single-user** developer tool — not a network service.

- The alias proxy binds **`127.0.0.1` only** (not exposed on the LAN).
- Upstream targets are always `127.0.0.1:{project.port}` from your config — no arbitrary host proxying.
- Writing `hosts` and binding port 80 may prompt **UAC**; approve only prompts you expect from Dev Tray.
- Treat `projects.json` as trusted (same machine / your repos). `id` / `alias` become hostnames and `command` is executed locally.
- Only use `id` / `alias` values you control (plain hostnames). Do not paste untrusted config that could poison the system `hosts` file.
- Dev servers behind the proxy have no extra auth — same exposure as opening `localhost:5173` directly on a shared machine.

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
  hosts/       Windows hosts block sync
  proxy/       local reverse proxy (Host → port)
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
