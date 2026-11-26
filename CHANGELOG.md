# Changelog

## 0.1.11

- Show popular OAuth providers and add visual color selection [skip ci]
- Connect AI tools without prompting for a route [skip ci]

## 0.1.10

- Test gateway package updates through the zcalld command [skip ci]
- Add a routing provider for virtual models

## 0.1.9

- Build the installer acceptance fixture with locked Cargo dependencies [skip ci]

## 0.1.8

- Fetch locked dependencies for GUI release packages [skip ci]

## 0.1.7

- Share Windows private-file checks and creation protection with zsys, retaining handle-based reads, owner/current-account/SYSTEM restrictions, non-inheriting creation and existing error messages. Route JSON string output through zconv while preserving control-byte spelling.
- Add bundled vector identities and display names for all 16 supported AI tools, with a neutral fallback for unknown tools and the same identity in the route picker.
- Use neutral light/dark surfaces and native interface typography; separate tool names, connection state, routes and actions.
- Add list paging and a compact layout for narrow windows, show missing tools as not installed, and remove inactive connect controls from detection-only rows.

## 0.1.6

- Repaint immediately after sidebar and in-page navigation so the selected page and visible content stay in sync without another input event.
- Keep source AUR builds offline by resolving the bundled toolkit locally, including on an empty Cargo cache.

## 0.1.5

- Reopen Windows core-installer recovery transactions whose protected directories use owner and SYSTEM inheritance, while retaining strict file ACL validation.
- Fetch locked Git dependencies before native CI checks and version bumps and update only workspace versions offline, preserving dependency commits and transaction rollback.

## 0.1.4

- fix(gui): use native Windows security for installer files [skip ci]

## 0.1.3

- feat(gui): install complete native bundles and guide first use [skip ci]
- fix: distinguish loading and failed GUI reads with safe retry [skip ci]

## 0.1.2

- docs: show the verified Ink interface in public previews
- Pin Ink GUI to the verified zgpui 0.1.4 rendering release
- ui: align the native control panel with the Ink palette
- build: consume the published zgpui 0.1.2 native fixes
- fix(aur): accept rustup's unversioned rust provider

## 0.1.1

- Use the unified zgpui native package and source submodule without a legacy dependency alias.

- Replace the oversized home emblem with a compact gateway status card and a focused setup action.
- Open provider setup or AI tools directly, with clickable local counts and fewer repeated controls.
- Use one new vector mark across the native window and Linux, Windows and macOS app icons.
- Capture isolated native previews only after the loaded frame has been presented.

## 0.1.0

- Add a native zui control panel for the zcalld JSON management interface.
- Keep provider credentials out of process arguments.
- Add bounded background CLI operations and on-demand rendering.
- Add SHA-256 verified GUI updates with one previous executable retained.
