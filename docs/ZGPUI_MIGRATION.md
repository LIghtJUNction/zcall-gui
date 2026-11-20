# Unified native UI dependency

The control panel now uses `zgpui::native` from the pinned
`vendor/zgpui` submodule. Its Cargo dependency requires version 0.1.1 and
enables the `native` feature. The application keeps its existing native
OpenGL path and window behavior.

## Source and package boundary

The committed gitlink selects the exact library source. The application
manifest, Cargo.lock, source archive, native archives, staged installation
and Arch source recipe all use the same dependency name and directory.
The standalone toolkit checks explicitly enable `native` because the
library's default feature set is empty.

The application version remains 0.1.0. This dependency migration does not
replace previously published tags or assets. A future application release
must pass its native release gates from a new immutable version.

## Validation scope

`python3 scripts/check.py --ui` checks formatting, strict Clippy, Rust
tests, release transactions, the optimized application, toolkit X11
properties and GUI interactions on private Xvfb displays. The release
packager must also produce a source archive containing the pinned toolkit
and a native archive containing its license.

Historical native-runner evidence in [VALIDATION.md](VALIDATION.md) retains
its original source revisions. The Linux results for this migration follow.

## Linux acceptance (2026-11-20)

The accepted library source is [eb8dbbe7d9a44abd9a8683dfa0588200a0595edb](https://github.com/LIghtJUNction/zgpui/commit/eb8dbbe7d9a44abd9a8683dfa0588200a0595edb),
consumed on top of application base `5dbc423eedd4d529a38b3e8e3adc3e6355a56b2b`.
The complete `python3 scripts/check.py --ui` command exited successfully.
Its [machine-readable record](evidence/zgpui-native-validation.json) contains
the actual window properties and private-fixture outcomes.

| Gate | Result |
| --- | --- |
| Application Rust tests | 15 passed |
| Toolkit Rust tests and doc tests | 30 + 1 passed with native enabled |
| Release transactions and AUR metadata | 11 passed |
| Strict Clippy, application and toolkit | Passed with warnings denied |
| Optimized application and toolkit probe | Built |
| X11 identity, UTF-8 title and requested geometry | 3 native windows passed |
| Application controls and settings persistence | Private fixture passed |
| Managed package-update result display | Private fixture passed; no system upgrade |

These results cover Linux x86_64. Windows/macOS native execution has not been
repeated for this dependency migration; the existing release gate still
applies before publishing new application binaries.
