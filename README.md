# zcall-gui

zcall-gui is an optional native control panel for [zcalld](https://github.com/LIghtJUNction/zcalld). It manages providers, AI tool settings, plugins, privacy rules, service state and updates by calling the installed `zcalld --json` interface. Closing the window leaves the gateway service running.

The application uses [zui](https://github.com/LIghtJUNction/zui) as a pinned Git submodule. Cargo resolves only these two first-party packages. Window creation, input, fonts and OpenGL use platform libraries; there is no embedded browser or third-party Rust crate.

## Build and run

```sh
git clone --recurse-submodules https://github.com/LIghtJUNction/zcall-gui.git
cd zcall-gui
cargo build --release --offline
./target/release/zcall-gui
```

Rust 1.100 and a native C compiler are required. On Arch Linux, install `base-devel rust pkgconf libx11 libgl freetype2 fontconfig`. On Ubuntu/Debian, install `build-essential pkg-config libx11-dev libgl1-mesa-dev libfreetype6-dev libfontconfig1-dev`. Linux Wayland sessions use XWayland.

The gateway is a separate installation. Use `zcall-gui --zcalld /absolute/path/to/zcalld --config /path/to/config.conf` to select a custom CLI or configuration. If the CLI is absent or an operation fails, the application displays the error; it does not silently create a second gateway.

## Controls

- **Overview:** inspect local status and start, stop or restart the gateway service.
- **Providers:** add or update an endpoint, select a protocol, set a default model, test it, or remove it. API keys are masked and sent to the CLI through stdin.
- **Agents:** detect supported tools, apply a gateway route, or revert an earlier configuration change.
- **Plugins:** inspect enabled state, install a local library or pinned remote source, enable/disable, update or remove a plugin. Newly installed native code requires a separate enable action.
- **Privacy:** select off, secret sanitization, or secret plus personal information sanitization.
- **Updates:** check and apply gateway or GUI updates.

Tab / Shift+Tab move focus, Enter activates buttons, Ctrl+A selects an input, and standard copy/paste shortcuts work. Ctrl+Q closes the GUI. Lists scroll independently from their adjacent configuration form.

## Updates

```sh
zcall-gui --update check
zcall-gui --update apply
```

The updater downloads only the official stable GitHub release for the current platform, checks SHA-256 and the staged executable's version, retains one previous binary, then replaces the installed executable. A user-writable installation can update in place. Files under system package directories use the package manager instead.

Linux and macOS keep the current process alive until you close it and open the new version. Windows uses a small deferred helper after the current process exits. The current first implementation is unsigned; Windows/macOS may require their normal downloaded-application approval.

## Development and evidence

Daily checks run locally:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --offline
xvfb-run -a -s '-screen 0 1100x740x24 -nolisten tcp' python3 scripts/ui_smoke.py
```

The Linux GUI has been opened on an Intel UHD GPU. The real-window test uses an isolated Xvfb display and a local CLI fixture to exercise provider stdin credentials, agent route changes, plugin activation, privacy selection and update checks. It does not call a remote model. Fixture screenshots show test data and are not evidence that a real gateway was configured.

The native backend currently uses X11/GLX on Linux, Win32/WGL on Windows and Cocoa/OpenGL on macOS. Apple has [deprecated OpenGL](https://developer.apple.com/documentation/appkit/nsopenglcontext); there is no Metal or native Wayland backend in this version. Native screen-reader accessibility and complex-script shaping are also not implemented.

License: [MIT](LICENSE).
