# ![Packrinth](/.github/assets/app_cover.png)

## Packrinth

Packrinth is an AI-powered Minecraft modpack maker built on top of the Modrinth App, an independent fork focused on AI-assisted workflows and a customizable workbench.

If you're not a developer and you've stumbled upon this repository, you can download the latest release of the app from the repository's releases page.

## Development

### Pre-requisites

Before you begin, ensure you have the following installed on your machine:

- [Node.js](https://nodejs.org/en/)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri](https://v2.tauri.app/start/prerequisites/)

### Setup

```bash
pnpm install
```

Copy the environment template and run the app:

```bash
cp packages/app-lib/.env.staging packages/app-lib/.env
pnpm app:dev
```

See [our goal specification](../../docs/goal.md) for the product specification and development plan.

## Branch strategy

- `upstream/main` (read-only mirror of the upstream Modrinth App repository)
- `main` (current stable, rebased on upstream/main)
- `develop` (development branch)

## Structure

- `apps/app` — Packrinth desktop shell (Tauri 2 + theseus)
- `apps/app-frontend` — Frontend UI (Vue 3)
- `packages/` — Shared libraries (theseus core, UI kit, api-client, and more)

## License

GPL-3.0-only for the app; see [COPYING.md](../../COPYING.md) and each package's LICENSE for details.
