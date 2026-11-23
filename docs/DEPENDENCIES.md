# Dependency propagation

## Unified native library

The GUI resolves `zgpui` from its public Git default branch. The application
enables `native` and imports `zgpui::native`; `Cargo.lock` and the
`vendor/zgpui` gitlink record the same verified source. Internal Git
declarations omit version, branch, tag and revision selectors.

The outer `zcalld` repository distributes this application through
`apps/zcall-gui`. Its source pin must follow the accepted GUI commit.
Project Z coordinates the library release and aggregate pins. The 0.1.2 consumer checkpoint
updated the manifest, lock and vendor gitlink in e6ed24b, with local unit,
Clippy and isolated native CLI-flow checks. The 0.1.3 integration introduced
window-local palette and font selection for Ink. The previous GUI snapshot
fixed the 0.1.4 rendering correction at
`7838c3c94cf018c02bb36e17c941ca651c5c0e24`. The toolkit uses power-of-two alpha textures where required
by older OpenGL implementations. Its official Windows GDI Generic/OpenGL 1.1
and macOS image gates verify glyph interiors and rounded corners. These are functional image checks, not a physical-GPU performance
measurement. The [official toolkit release gate](https://github.com/LIghtJUNction/zgpui/actions/runs/44710634988)
records that platform coverage. The default toolkit appearance remains unchanged
for other applications. The
[unified-library acceptance record](ZGPUI_MIGRATION.md) documents the local
validation scope.

The current lockfile and vendor gitlink select `zgpui` 0.1.8. The latest
source adds an optional `bridge` feature; this application continues to
enable `native` without `bridge`. The native implementation and build script
are unchanged by that source update. The earlier cross-platform release
evidence above describes its original source and remains a separate record
from current dependency validation.

Checksum verification delegates SHA-256 to RustCrypto `sha2` and lowercase
hexadecimal encoding to `zhex`. Both GUI self-updates and gateway helper
verification call the same application wrapper. The GUI keeps its bounded
download and installation policy, while the cryptographic and encoding
implementations live in their independent libraries. `zhex` follows the same
internal Git source rule; the external `sha2` crate uses a registry dependency.

The existing `v0.1.3` tag remains fixed at
`651aee3dc1eb43b561baafa77ce0ed12d9f3b2ff`, which uses zgpui 0.1.4.
The 0.1.5 dependency change belongs to development source after that tag.
The native release workflow checks out the requested tag for both build and
publication, and verifies that checkout against the tag before packaging.
Advancing `main` does not change this frozen release source or its artifacts.

## Historical native-library correction

The record below describes the earlier zui snapshot and its original
acceptance. It is retained as provenance; the active dependency is defined
above and in the committed manifest, lockfile and gitlink.

The GUI freezes zui at 5e37fcfb791eec9ac523fc19373e95d0a513112f.
Its runtime correction is 9eac4c9a2232b6cdb82a29bf6adf94f96e9412bb; the final
commit adds maintenance instructions and does not change runtime sources.

Cargo resolves zui-native 0.1.0 through vendor/zui and imports it as zui.
The package version and Cargo.lock entry remain unchanged in this pre-release
source correction. The Git submodule records the exact implementation.

### Historical impact and ownership

The observed Project Z impact command returned zcall-gui, ztype and zui.
The aggregation checkout was at 32e5e82685e107b5b1f85a32d054922a0b8cbfa6
with working-tree changes at observation time. The outer zcalld repository
also distributes this GUI as a source submodule, so that relationship must
be registered even though the core executable does not link the GUI.

| Consumer | Required propagation | Receiving owner |
| --- | --- | --- |
| zcall-gui | Pin vendor/zui to the frozen commit; preserve the exact path package and lock entry; native tests and package validation | GUI maintainer |
| ztype | Pin the same zui source; check manifest/lock; verify its compact window and native/source-package checks; push its result | Existing ztype maintainer through Project Z coordination |
| zcalld | Pin apps/zcall-gui to this final GUI snapshot; verify its source distribution and management integration; push its result | Existing zcalld maintainer |
| Project Z aggregation | Register the outer source-submodule edge; update repository pins and lock/manifest; verify impact and consistency; push acceptance records | Existing Project Z maintainer |

The GUI side has verified real CLI actions in an isolated home, native update
command wiring, the rebuilt 1080 by 740 window, and actual X11 class/size/UTF-8
properties. The shared probe also verifies ztype's 660 by 264 dimensions.
Native release gates test Linux x86_64/aarch64, Windows x86_64 and macOS
arm64/x86_64 before public artifacts are published.

Native C objects inherit CFLAGS. The GUI's Arch source recipe disables
makepkg C LTO while retaining Rust thin LTO; the real source-package build and
check succeeded with that setting. Downstream source-package owners must
check their own compiler/linker combination.

This file records the frozen snapshot and handoff boundaries. Final downstream
commit IDs, test results, and any concrete blocker with its receiving owner
belong in the coordinating Project Z acceptance record and delivery report.
They must be collected; publication or notification alone is not completion.
Keeping acceptance records outside this source-pin chain avoids repeatedly
changing the library or GUI merely to record a consumer's acceptance.

Project Z is the sole crates.io publisher for zui-native 0.1.0.
