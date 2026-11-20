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

This dependency migration preserves previously published tags and assets.
New application releases pass their native release gates from a new immutable
version.

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

## Final release source

The final gitlink is [cb50f2fe554622fdc984ae7fec9bcd844f175d67](https://github.com/LIghtJUNction/zgpui/commit/cb50f2fe554622fdc984ae7fec9bcd844f175d67).
Its `src/`, `native/`, `build.rs` and `Cargo.lock` are byte-identical
to the runtime tested above. The manifest changes only the package inclusion
list. [The source comparison](evidence/zgpui-release-source.json) separates
this final release-source pin from the earlier executed test revision.

## Redesigned application integration

The native dependency migration was combined with application UI commit
`93ab4770b66bbea117fad88a4b5a8c92a3854b3c`, preserving the simplified overview,
shared vector identity and native preview readiness checks. The merged source
passed `python3 scripts/check.py --ui` with the exact released toolkit
`cb50f2fe554622fdc984ae7fec9bcd844f175d67` (zgpui v0.1.1).

The [combined record](evidence/redesigned-native-validation.json) includes
15 application tests, 30 toolkit tests, one doc test, 11 release-transaction
tests, four real X11 property checks and private GUI interaction/update checks.
It does not turn the private llvmpipe display into a hardware-GPU benchmark.
The formal application release repeats the required native platform gates.
