# Validation scope

This record distinguishes application behavior, native display evidence and software rendering measurements. All GUI input automation used a private Xvfb display. The user's desktop pointer, existing AI tool configuration and running Magpie service were not changed by these GUI tests.

## Verified behavior

The actual developing zcalld command-line program was exercised with an isolated home and explicit configuration. The native window added a provider, sent its test credential through stdin, confirmed the saved provider through a fresh CLI read, applied and restored a Claude tool configuration, and changed privacy to personal. It also saved Japanese and dark appearance, then opened a second process that restored the saved dark theme. The run recorded 40 CLI invocations. See [the raw result](evidence/real-cli-ui.json) and [the test](https://github.com/LIghtJUNction/zcall-gui/blob/main/scripts/ui_smoke.py).

The fixture run additionally covers plugin enabling and gateway update checking. These fixture actions validate command wiring, not live upstream availability or actual update installation. No test invoked a remote model.

Rust tests cover bounded JSON parsing, Unicode handling, known SHA-256 vectors, catalog completeness and locale selection. zui tests cover UTF-8 editing and widget bounds. Clippy passed for both packages. Release transaction tests cover dry runs, exact refs, existing tags, rollback after lockfile failure and AUR downgrade refusal.

## Package-manager update action

The native Updates page was exercised on a private Xvfb display using inert pacman, paru and terminal fixtures. A nonzero helper exit produced an error state. A successful helper exit refreshed actual command-line state and displayed the installed executable version. The test verified literal -Syu arguments, ignored a supplied JSON command string and checked removal of private result files. It did not upgrade the development machine. See [the raw result](evidence/managed-update.json) and [the native interaction test](../scripts/managed_update_smoke.py). Rust tests additionally cover source and binary package ownership, yay fallback, detached terminal launchers and rejection of unrelated package owners.

## Window and renderer evidence

Earlier builds opened on the development machine's Intel UHD GPU through XWayland. The renderer identified itself as Mesa Intel(R) UHD Graphics (ADL-S GT1). The current screenshots use a private 1080 by 740 Xvfb display with llvmpipe to keep the entire window visible and exclude other desktop applications.

During the final geometry pass, the development session's DISPLAY=:0 X11 socket was unavailable, so hardware measurements could not be repeated for the final binary. The existing compositor and its sockets were not modified. Earlier Intel measurements must not be presented as measurements of the current build.

The final screenshot pages are [Overview light](screenshots/overview-light.png), [Overview dark](screenshots/overview-dark.png), [Providers light](screenshots/providers-light.png) and [Providers dark](screenshots/providers-dark.png). They show actual CLI state from an isolated test configuration; an inactive gateway is displayed as disconnected.

## Software-renderer measurement

The measured local 0.1.0 development binary was 690,456 bytes. In the recorded private Xvfb/llvmpipe run, process RSS was 107,408 KiB and PSS was 89,879 KiB. Over a 3.0016 second idle interval, the process accumulated 0 CPU ticks at a 100 Hz accounting resolution. It presented 3 frames across the entire bounded run. See [the raw measurement](evidence/software-renderer.json) and [the measurement script](https://github.com/LIghtJUNction/zcall-gui/blob/main/scripts/measure_idle.py).

These numbers include software-renderer and system-font costs. They are not a hardware-GPU memory budget, a dedicated-VRAM measurement, or proof of zero CPU use at every instant. The app does not run an animation loop or periodic gateway requests. It redraws on input, state changes or window events, and refreshes gateway state on explicit refresh or a completed action.

## Platform status

Linux compile, real-window actions and the earlier Intel display path are verified locally. Windows and macOS source implementations remain subject to the native release-candidate workflow before the first release is published. The reusable platform gate records results for Linux x86_64/aarch64, Windows x86_64 and macOS arm64/x86_64.

Native screen-reader support, a direct Wayland backend, Metal rendering and full complex-script text shaping are outside this first implementation.
