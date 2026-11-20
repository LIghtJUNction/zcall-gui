# Dependency propagation

## Unified native library

The active GUI dependency is `zgpui =0.1.1`, stored as the
`vendor/zgpui` Git submodule. The application enables `native` and imports
`zgpui::native`. The package name, Rust dependency key and source directory
use the same name; there is no legacy package alias.

The outer `zcalld` repository distributes this application through
`apps/zcall-gui`. Its source pin must follow the accepted GUI commit.
Project Z coordinates the library release and aggregate pins. The
[unified-library acceptance record](ZGPUI_MIGRATION.md) documents the local
validation scope.

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
