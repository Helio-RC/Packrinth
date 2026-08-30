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

**Docs**

- [pnpm commands](docs/pnpm-commands.md) — every pnpm script and what it does
- [Build & CI](docs/build-ci.md) — manual build steps and GitHub Actions pipeline
- [i18n](docs/i18n.md) — locale storage, extraction, and Crowdin sync
- [Repo config](docs/repo-config.md) — required secrets and variables for Actions

## Branch Strategy

- `upstream/main` — tracking the upstream Modrinth monorepo
- `main` — our integration branch, rebased from `upstream/main`
- `develop` — active development

To sync with upstream:

```bash
git fetch upstream
git rebase upstream/main # on main, then re-rebase develop
```

## Goal

For the product vision and roadmap, see [`docs/goal.md`](docs/goal.md).
