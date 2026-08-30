# Copying Guidelines

Root of the repository does not carry a LICENSE file; each package/app carries its own (see `LICENSE`/`COPYING.md` in each package or app directory; the desktop app under `apps/app/` is GPL-3.0-only).

For detailed information, consult each package's COPYING.md, LICENSE.txt, or LICENSE file, if available.

## Upstream and attribution

Packrinth is an independent fork of the Modrinth App. The upstream source is [modrinth/code](https://github.com/modrinth/code), © 2020-2025 Rinth, Inc., licensed under the GNU General Public License, version 3. Upstream code retains its upstream copyright and GPL-3.0 licensing.

Product branding (name, window title, installer text, and UI strings) has been replaced with Packrinth. No Modrinth trademark or logo is distributed in Packrinth's released builds _except_ the placeholder assets listed below, which still use upstream Modrinth artwork pending replacement before the first public release. Where upstream code or assets are retained, they remain subject to their respective upstream licenses and copyright.

> All rights reserved. © 2020-2025 Rinth, Inc. (upstream)

## Residual branding assets

The following files still ship upstream Modrinth logo artwork and must be replaced or re-licensed before the first public release of Packrinth:

- .idea/icon.svg
- apps/app/icons/\* (including `apps/app/icons/apple.icon/`)
- apps/app/dmg/dmg-background.png
- apps/app-frontend/src/assets/welcome/modrinth-social-icon.png
- apps/app-frontend/src/assets/modrinth_app.svg (unreferenced)
- apps/app-frontend/src/assets/sad-modrinth-bot.webp (unreferenced)
- apps/app-frontend/src/components/ui/SplashScreen.vue (inline logo SVG)

Plus the repository cover asset:

- .github/assets/app_cover.png

> **TODO:** Replaced or re-licensed before public distribution.
