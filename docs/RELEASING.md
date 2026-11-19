# Releasing zcall-gui

Releases start from a clean, committed tree with the zui submodule at its recorded revision. The app version lives in Cargo.toml and Cargo.lock. The toolkit has its own package version and release history.

## Local gate

~~~sh
git submodule update --init --recursive
make check
# For an interaction gate instead (includes the checks above):
make check-ui
~~~

The first command builds and checks both Rust packages, tests JSON/Unicode/checksum/catalog behavior and exercises release transactions. The UI gate opens a real native window on a private Xvfb display. To repeat the actual CLI integration with an isolated home:

~~~sh
xvfb-run -a -s '-screen 0 1080x740x24' \
  python3 scripts/ui_smoke.py target/release/zcall-gui \
  --real-cli /absolute/path/to/zcalld --output _validation/real-cli
~~~

Normal commits do not trigger CI. The manual Release candidate workflow validates a full commit SHA on five native runners. It creates build artifacts and does not publish a GitHub release or AUR recipe:

~~~sh
gh workflow run check.yml --repo LIghtJUNction/zcall-gui
~~~

The native gate covers Ubuntu 24.04 x86_64 and aarch64, Windows x86_64 with MSVC, and macOS arm64 and x86_64. Linux includes a private-display interaction test; Windows and macOS open a bounded native smoke window. Windows/macOS artifacts are not code signed or notarized.

## Fast version bump

Preview a patch release:

~~~sh
gh workflow run bump.yml --repo LIghtJUNction/zcall-gui \
  -f bump=patch -f dry_run=true
~~~

Create the version commit and release:

~~~sh
gh workflow run bump.yml --repo LIghtJUNction/zcall-gui -f bump=patch
~~~

The same workflow accepts minor, major, or custom plus version=1.2.3. It updates the app manifest, lockfile and changelog, creates an annotated tag, and atomically pushes the version commit with the tag. It explicitly invokes the reusable release workflow, so it does not depend on a second workflow being triggered by a GITHUB_TOKEN tag push.

For a local release:

~~~sh
python3 scripts/release.py bump patch --dry-run
python3 scripts/release.py bump patch
make check
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: zcall-gui 0.1.1"
git tag -a v0.1.1 -m "zcall-gui 0.1.1"
git push --atomic origin main v0.1.1
~~~

Use the version produced by the bump command. Existing tags, non-increasing versions, dirty submodules and a tag pointing at another commit are rejected. If lockfile generation fails, the local version transaction restores the original files.

## Artifacts and installers

Each platform has a raw executable and a matching .sha256 file for the built-in updater, plus a native archive. Linux archives contain the executable, desktop entry, SVG icon, licenses, documentation and a staged installer. Windows uses a ZIP with a branded executable; macOS uses a tarball containing an app bundle and ICNS icon.

The source archive includes the exact zui submodule source. The ordinary GitHub-generated source ZIP does not include that Git submodule and is not the input used by the AUR source recipe.

Download the matching archive and SHA256SUMS from the [GitHub release](https://github.com/LIghtJUNction/zcall-gui/releases). Official Linux binaries are built on Ubuntu 24.04; older distributions may need a source build. For a Linux user installation:

~~~sh
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf zcall-gui-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
cd zcall-gui-v0.1.0-x86_64-unknown-linux-gnu
./install.sh
~~~

The default prefix is ~/.local. Put ~/.local/bin on PATH before starting the desktop entry. Set PREFIX=/usr/local for another prefix, or combine PREFIX=/usr and DESTDIR=/path/to/stage when validating a distribution package. The installer does not start or configure a gateway service.

## AUR

The source package is zcall-gui; the prebuilt package is zcall-gui-bin. Both require zcalld 0.2.0 or newer. A versioned zcalld-bin package can satisfy that dependency. On Arch:

~~~sh
yay -S zcall-gui-bin
# Or compile the released source:
yay -S zcall-gui
~~~

If an AUR helper has not indexed a newly created package, use the canonical recipe directly:

~~~sh
git clone https://aur.archlinux.org/zcall-gui-bin.git
cd zcall-gui-bin
makepkg -si
~~~

The release publisher downloads the public GitHub assets again, calculates their SHA-256 hashes, and renders both PKGBUILD and .SRCINFO. It never publishes SKIP hashes. A newer AUR epoch, package version or package revision is preserved when an older GitHub release is retried.

Automatic AUR publishing requires repository variable AUR_PUBLISH=true and secrets AUR_SSH_KEY and AUR_KNOWN_HOSTS. The latter must contain an already verified host key. No crates.io token is needed to release the app.

## Recovery

A manual release run accepts an existing immutable tag:

~~~sh
gh workflow run release.yml --repo LIghtJUNction/zcall-gui -f tag=v0.1.0
~~~

The publisher resumes missing uploads in a draft and refuses to replace an existing asset with different bytes. Published tags and assets remain immutable. If source changes are required, publish a new version.

If GitHub publishing succeeded but AUR publishing failed, retry only the AUR step from the exact release checkout after correcting the operational cause:

~~~sh
git switch --detach v0.1.0
git submodule update --init --recursive
python3 scripts/aur.py --version 0.1.0 --publish
~~~

Use a fresh ignored _release/aur checkout for the retry. The script does not force push or rewrite AUR history.

## Evidence

[Validation](VALIDATION.md) distinguishes real CLI actions, native display evidence, software-renderer measurements and platform gates. Release workflow results provide the evidence for the exact tagged artifacts; local development measurements describe only the binary hash recorded with them.


## Updating an AUR installation from the app

On the Updates page, check for a release, then select **Update in terminal**.
The app asks pacman which package owns the selected executable and preserves
the source or binary package choice. It launches the installed paru or yay
with **-Syu** and that package name in a native terminal. This is a normal Arch
system upgrade: review the package list and authorize it in the terminal.
No confirmation is bypassed. The app displays the helper exit status and reads
the installed executable version after success; reopen the app after updating
its own package. The running gateway service is controlled separately.

Install paru or yay and a terminal emulator to use this button.
The default terminal can be selected with xdg-terminal-exec or a recognized
TERMINAL executable. If no helper or terminal exists, the app reports the
missing prerequisite. Portable installations continue to use the verified
download and rollback updater.
