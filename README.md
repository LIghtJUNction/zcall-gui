# zcall-gui

zcall-gui is an optional native control panel for [zcalld](https://github.com/LIghtJUNction/zcalld). It manages providers, AI tool settings, plugins, privacy rules, usage, service state and updates by calling the installed `zcalld --json` interface. Closing the window leaves the gateway service running.

The application uses [zgpui](https://github.com/LIghtJUNction/zgpui) 0.1.5, with its public Git source pinned in `Cargo.lock` and the same revision recorded by the `vendor/zgpui` submodule. Cargo resolves only these two first-party packages; the GUI enables the toolkit's `native` feature and imports `zgpui::native`. Window creation, input, fonts and OpenGL use platform libraries; there is no embedded browser or third-party Rust crate.

## Build and run

```sh
git clone --recurse-submodules https://github.com/LIghtJUNction/zcall-gui.git
cd zcall-gui
cargo fetch --locked
cargo build --release --locked --offline
./target/release/zcall-gui
```

Rust 1.100 and a native C compiler are required. On Arch Linux, install `base-devel rust pkgconf libx11 libgl freetype2 fontconfig`. On Ubuntu/Debian, install `build-essential pkg-config libx11-dev libgl1-mesa-dev libfreetype6-dev libfontconfig1-dev`. Linux Wayland sessions use XWayland.

Management commands require zcalld 0.2.0 or newer. Use `zcall-gui --zcalld /absolute/path/to/zcalld --config /path/to/config.conf` to select a custom CLI or configuration. A missing CLI presents **Install background service**. Installation requires an explicit review and confirmation; opening the GUI never downloads or starts a gateway.

## Install the gateway

The GUI obtains a complete native gateway from an official stable core release. It first verifies the small `zcalld-installer` helper against that release's SHA-256 list and checks its version and supported installer protocol. The same helper implements the core CLI's `zcalld install` commands. It validates and stages the full archive, including the gateway, native plugins and platform helpers, before changing an owned installation.

The confirmation page shows the version, installation location and administrator requirement. New installations use a private location and do not request administrator privileges:

| Platform | Default location |
| --- | --- |
| Linux | `$XDG_DATA_HOME/zcalld/runtime`, or `~/.local/share/zcalld/runtime` |
| macOS | `~/Library/Application Support/zcalld/runtime` |
| Windows | `%LOCALAPPDATA%/zcalld/runtime` |

After a committed transaction, the GUI remembers the actual executable path. It also discovers the default private installation on later launches, so editing `PATH` is not required. An explicit `--zcalld` or `ZCALLD_BIN` still takes precedence. Unknown shared or legacy directories are not adopted; an unmanaged legacy executable can be left in place while a new private installation is prepared.

Installation leaves service control and user configuration separate. For a committed private installation whose selected configuration is explicitly reported missing, the next action is **Initialize configuration**. It invokes the core's existing `init` command after a fresh state check; an existing or unreadable file is never treated as missing. The confirmation identifies the configuration location. No service is started by initialization.

On Linux, **Install user service** then verifies or creates the user unit for that exact executable and configuration. A pre-existing unrelated unit is an error, even if the service manager reports a unit with the same name. Only after this explicit check does the bootstrap flow offer **Start gateway**. On macOS and Windows, the core's existing service-start registration remains a separate explicit action. Opening the GUI, preparing a bundle and completing its installation do not start or stop a service, refresh an account, install a model or rewrite an AI tool's configuration.

## Controls

- **Overview:** see the gateway state and one next step: add a provider or connect your AI tools. Click either count to open its list; service controls stay in Service settings.
- **Providers:** search by name, model, protocol or endpoint; add or update an endpoint, select a protocol, set a default model, test it, or remove it. API keys are masked and sent to the CLI through stdin.
- **AI tools:** detect supported tools, apply a gateway route, or revert an earlier configuration change.
- **Plugins:** inspect enabled state, install a local library or pinned remote source, enable/disable, update or remove a plugin. Newly installed native code requires a separate enable action.
- **Privacy:** select off, secret sanitization, or secret plus personal information sanitization.
- **Usage:** inspect recorded upstream attempts, including retries, successful and failed attempts, input/output tokens, cache and reasoning details, and attempt success rate. The page reads local accounting on first open or when you select Refresh; other pages do not scan usage history. Enable or disable local recording from this page, then explicitly restart the service to apply the saved setting. Disabling recording preserves existing history. Older gateways without detail counters still show their supported totals; a missing recording-status field is shown as unknown.
- **Updates:** install a missing gateway, update a complete unmanaged gateway, inspect or recover an installation, or use the package manager for a managed installation. GUI updates remain separate.

Tab / Shift+Tab move focus, Enter activates buttons, Ctrl+A selects an input, and standard copy/paste shortcuts work. Ctrl+Q closes the GUI. Provider rows open details on demand. Adding an endpoint uses two short steps; removal stays in the detail panel.

## Appearance and language

The window uses neutral light or graphite surfaces, restrained blue accents and
the platform's native interface font with glyph fallback. Controls, input focus
and status colors share the application-local palette. It does not install
fonts or change desktop fonts or other applications.

The AI tools list gives each of the 16 supported tools its own vector mark and
display name. Connection state and the current or saved route appear separately.
Detection-only tools have no inactive connect button. Narrow windows move route
actions below the tool details, and paging controls complement wheel scrolling.
The route picker repeats the selected tool's mark and name. All icon geometry is
bundled; displaying this page requires no network request. [Asset sources and
licenses](assets/agents/README.md) record the original SVGs and native conversion.

Open Settings at the lower left to select System, Light or Dark, and System, Simplified Chinese, Traditional Chinese, English or Japanese. Choices are stored locally and restored on the next launch. System preferences are read at launch and when opening Settings; there is no background desktop-preference polling.

Language strings live in four complete catalogs under resources/locales. Tests check matching keys and locale fallback. An explicit session override is also available:

~~~sh
zcall-gui --theme dark --language ja
~~~

The previews use synthetic local configuration in a private native window.

[Overview in light mode](docs/evidence-archive-20261120.md)

[Providers in dark mode](docs/evidence-archive-20261120.md)

## Updates

### Gateway and native plugins

The gateway update button prepares a fixed official release and displays its confirmation. Its saved plan is bound to the release commit, platform, archive checksum and installation location. The GUI rechecks the downloaded helper and approved plan before invoking them. Verification and recovery paths are available in **Details**.

If an owned gateway is still running, the installation stays at **Waiting for service exit**. Stop it through the existing service controls, then use **Complete update** or **Check status**. A launched helper or pending transaction is never reported as an installed update. Closing the window does not cancel a helper already performing a transaction; reopening reads its local result. The prior installation and recovery journal are retained for **Restore previous**.

The same GUI entry point is available without opening a window:

```sh
zcall-gui --core-install prepare
zcall-gui --core-install apply
zcall-gui --core-install status
zcall-gui --core-install resume
zcall-gui --core-install rollback
```

These commands return JSON. `prepare` obtains the official metadata and verified helper; `apply` is the explicit installation action. `status` only reads local installation state. Native bootstrap needs system `curl` and `tar`; Windows uses native file-handle security APIs to set and verify private cache permissions. The GUI and installer add no third-party Rust dependency or Python runtime requirement. A core release without the complete archive and installer protocol is rejected before installation.

Managed paths remain on their package-manager update route. For AUR packages, the existing native terminal preserves package review, authorization, source-versus-binary package identity, and actual exit status.

### GUI executable

```sh
zcall-gui --update check
zcall-gui --update apply
```

The GUI self-updater downloads only its official stable GitHub release for the current platform, checks SHA-256 and the staged executable's version, retains one previous GUI binary, then replaces that executable. A user-writable GUI can update in place. Managed GUI packages use their package manager.

Linux and macOS keep the current process alive until you close it and open the new version. Windows uses a small deferred helper after the current process exits. The current first implementation is unsigned; Windows/macOS may require their normal downloaded-application approval.

## Releases and packages

Published binaries and archives are available through [GitHub Releases](https://github.com/LIghtJUNction/zcall-gui/releases). Arch recipes are zcall-gui (source) and zcall-gui-bin (binary), both requiring zcalld 0.2.0 or newer. [Release and installation instructions](docs/RELEASING.md) cover AUR, portable archives, staged installs and the patch-bump workflow.

## Development and evidence

Daily checks run locally:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --offline
xvfb-run -a -s '-screen 0 1100x740x24 -nolisten tcp' python3 scripts/ui_smoke.py
xvfb-run -a -s '-screen 0 1100x740x24 -nolisten tcp' python3 scripts/usage_recording_smoke.py
xvfb-run -a -s '-screen 0 1440x1000x24 -nolisten tcp' python3 scripts/agent_identity_smoke.py
```

The Linux GUI has been opened on an Intel UHD GPU. The real-window test uses an isolated Xvfb display. It exercises provider stdin credentials, agent route changes, plugin activation, privacy selection and update checks against a local fixture. A separate run uses the actual developing zcalld CLI and an isolated home to verify saved providers, agent apply/restore, privacy changes and appearance persistence. Neither run calls a remote model. Screenshots contain isolated test configuration. [Validation evidence](docs/VALIDATION.md) records the scope and current limits. [The 0.1.4 release record](docs/validation-0.1.4.md) identifies the exact source, native jobs and published archives.

The recording smoke uses a stateful synthetic CLI and a 45-second native window. It checks that enabling recording only saves configuration, restarting requires confirmation, successful restart clears the pending state, and disabling preserves the displayed history before and after another confirmed restart. It saves seven screenshots and a JSON result under `_validation/usage-recording`; it does not operate a real service.

The native backend currently uses X11/GLX on Linux, Win32/WGL on Windows and Cocoa/OpenGL on macOS. Apple has [deprecated OpenGL](https://developer.apple.com/documentation/appkit/nsopenglcontext); there is no Metal or native Wayland backend in this version. Native screen-reader accessibility and complex-script shaping are also not implemented.

License: [MIT](LICENSE).

[Dependency propagation](docs/DEPENDENCIES.md) records the frozen shared-library snapshot and downstream ownership.
