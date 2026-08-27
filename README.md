# Packrinth

Packrinth is an AI-powered modpack maker, built as a fork of the [Modrinth App](https://modrinth.com/app). It lets you describe your ideal modpack and assemble it automatically.

## Tech Stack

- **Tauri 2** — desktop shell
- **theseus** — Modrinth's core app library (`packages/app-lib`)
- **Vue 3** — frontend (`apps/app-frontend`), shared components in `packages/ui`

## Development

```bash
pnpm install
pnpm app:dev
```

Copy the `.env` template in `packages/app-lib/` before the first run.

## Branch Strategy

- `upstream/main` — tracking the upstream Modrinth monorepo
- `main` — our integration branch, rebased from `upstream/main`
- `develop` — active development

## Goal

For the product vision and roadmap, see [`apps/app/docs/goal.md`](apps/app/docs/goal.md).