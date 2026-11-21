# Agent identity assets

These marks identify tools in the AI tools list and route picker. The backend's
stable ID remains the argument passed to `zcalld`; the friendly name and mark
affect presentation only. Unknown IDs retain their supplied name and use a
neutral terminal symbol.

`sources.json` records each source URL, immutable repository revision when one
exists, SHA-256, license, and any normalization. The original SVGs remain beside
the normalized inputs. All assets and generated meshes are text files.

| Tools | Source | Asset license |
| --- | --- | --- |
| Claude Code, Codex, Gemini CLI, OpenCode, Qwen Code, goose, Cursor, Amp, Cline, Roo Code, Kilo Code | [Lobe Icons](https://github.com/lobehub/lobe-icons/tree/9752d967494e48d8f83f2808feb2b0207bf70997/packages/static-svg/icons) | [MIT](LICENSES/LobeHub-MIT.txt) |
| Pi | [Official press kit](https://pi.dev/press-kit) | [MIT](LICENSES/Pi-MIT.txt) |
| aider | [Official repository asset](https://github.com/Aider-AI/aider/blob/5dc9490bb35f9729ef2c95d00a19ccd30c26339c/aider/website/assets/icons/safari-pinned-tab.svg) | [Apache-2.0](LICENSES/Aider-Apache-2.0.txt) |
| Continue | [Official repository asset](https://github.com/continuedev/continue/blob/5522c6f44ca0ac3528b37244818fbfa39b5af470/extensions/intellij/src/main/resources/icons/continue.svg) | [Apache-2.0](LICENSES/Continue-Apache-2.0.txt) |
| Zed | [Simple Icons](https://github.com/simple-icons/simple-icons/blob/32e030086fdc605196391aab28519b14dc4295b4/icons/zedindustries.svg) | [CC0-1.0](LICENSES/Simple-Icons-CC0-1.0.txt) |
| Visual Studio Code | [Microsoft's official icon package](https://code.visualstudio.com/brand) | Microsoft brand asset; not covered by this project's MIT license |

The Microsoft source is retained unchanged as `vscode.svg`; its alternate
monochrome silhouette is recorded in `normalized/vscode.svg`. The normalization
preserves path geometry and aspect ratio and omits shadows and gradient
overlays. Pi and Continue are scaled uniformly and use theme-aware ink. Names
and marks belong to their respective owners; their appearance identifies the
corresponding tool and does not indicate endorsement of zcall-gui.

## Native rendering

`src/agent_logos.rs` contains triangles generated from the selected filled SVG
silhouettes. This preserves holes and concave outlines while using the native
toolkit's convex-polygon primitive. Coordinates use a 1024-unit square. The
generator checks the difference between the SVG silhouette and the quantized
triangles, including viewport clipping. Colors are chosen by the application for
the current theme; each tool also has a visible text name and status.

Normal Cargo builds read the committed Rust mesh data. They do not fetch icons,
parse SVG, invoke Python, or add a Rust dependency. To regenerate after an asset
change, install the development-only packages and run:

```sh
python3 -m pip install svgpathtools shapely numpy mapbox-earcut
python3 scripts/generate_agent_logos.py
cargo fmt
```

Review the source manifest and native screenshots whenever an input changes.
The generator accepts reviewed filled outlines; it rejects unsupported masks,
filters, text, external references, and strokes instead of silently dropping
them.
