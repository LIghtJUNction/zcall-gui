# zcall-gui 0.1.4 release validation

This record covers the immutable `v0.1.4` release at [`190d9e355f44e3cd0bd785f434ebbd6b244e13be`](https://github.com/LIghtJUNction/zcall-gui/commit/190d9e355f44e3cd0bd785f434ebbd6b244e13be). The release was published on 2026-11-20 at 16:41:01 UTC. It does not claim that any user's installed application has already been upgraded.

## Source and dependency identity

The application version is 0.1.4. Its only Rust dependency is the first-party `zgpui` package, version 0.1.5, at the exact submodule commit [`b4d5493e0cd985d00558c7be0b5726965ab5163a`](https://github.com/LIghtJUNction/zgpui/commit/b4d5493e0cd985d00558c7be0b5726965ab5163a). This is the source pin used by the release; it should not be confused with the toolkit's earlier version tag. The recursive source archive carries that pinned dependency.

The GUI and toolkit use platform window, font and graphics libraries. “No third-party Rust dependency” does not mean that the operating system provides no runtime libraries.

## Native validation

The [candidate run](https://github.com/LIghtJUNction/zcall-gui/actions/runs/44768446366) and [formal release run](https://github.com/LIghtJUNction/zcall-gui/actions/runs/44775346541) both used the exact application commit above. Both completed successfully on all five native targets:

| Runner | Target |
| --- | --- |
| Ubuntu x86_64 | `x86_64-unknown-linux-gnu` |
| Ubuntu ARM64 | `aarch64-unknown-linux-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS ARM64 | `aarch64-apple-darwin` |
| Windows x86_64 | `x86_64-pc-windows-msvc` |

The Windows candidate passed 37 application unit tests, 33 toolkit unit tests, one toolkit doctest and all nine bootstrap scenarios. The bootstrap scenarios completed in 5.213 seconds without skipping a scenario or increasing their deadlines. Their native owner/DACL, link and locking checks exercise the Windows implementation. The separate publishing-helper fixture has an Arch-only skip on Windows.

The Windows hosted runner reported GDI Generic. That result verifies the available rendering path and does not establish hardware GPU performance on Windows. The release jobs are correctness and packaging evidence, not throughput, peak-memory or end-user GPU benchmarks.

## Published artifacts

The [formal release](https://github.com/LIghtJUNction/zcall-gui/releases/tag/v0.1.4) contains the platform executables, platform archives, recursive source archive and `SHA256SUMS`. The recorded GitHub SHA-256 asset digests include:

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `zcall-gui-v0.1.4-source.tar.gz` | 480777 | `2d897aaba3f593c8f9fecc43caba90d725b08a85bb4f15a1d026d889a30d2f1c` |
| `zcall-gui-v0.1.4-x86_64-unknown-linux-gnu.tar.gz` | 634336 | `103e27a0e7398abb0dce39b6f69638b263be4ecb5ceb0ea8a04ed358157ae7ed` |
| `zcall-gui-v0.1.4-x86_64-pc-windows-msvc.zip` | 658352 | `9514db123f2f0bc65e0eb1ad0519c50ada4dcb178b39dc9087e6fd6e7ec8512c` |

The formal publish job also completed its AUR recipe publication. The observed source-package AUR master was `d4cf10b291d443014618ad05a03e038c14c56b0e`. Recipe publication is distinct from a clean Arch source build and from installing a package on a device.

The [machine-readable release receipt](evidence/zcall-gui-0.1.4.json) retains all asset digests and native job identities. Later documentation changes on `main` do not move the release tag or replace its assets.
