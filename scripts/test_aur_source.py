#!/usr/bin/env python3
"""Check the real source AUR prepare step without a Git or registry cache."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib

import aur
from release import ROOT, current, validate


def lock_graph(path):
    packages = tomllib.loads(path.read_text())["package"]
    return sorted((p["name"], p["version"], tuple(sorted(p.get("dependencies", []))))
                  for p in packages)


def metadata(tree, manifest, env, expected, features=()):
    command = ["cargo", "metadata", "--format-version", "1", "--frozen",
               "--manifest-path", str(manifest)]
    if features:
        command.extend(["--features", ",".join(features)])
    result = subprocess.run(
        command, cwd=tree, env=env,
        check=True, text=True, stdout=subprocess.PIPE, timeout=120,
    )
    data = json.loads(result.stdout)
    actual = sorted((p["name"], p["version"]) for p in data["packages"])
    if actual != sorted((name, version) for name, version, _ in expected):
        raise ValueError("Frozen metadata changed the bundled package versions")
    locked = {(p["name"], p["version"]): p.get("source")
              for p in tomllib.loads((manifest.parent / "Cargo.lock").read_text())["package"]}
    for package in data["packages"]:
        path = Path(package["manifest_path"]).resolve()
        if not path.is_relative_to(tree):
            raise ValueError("Frozen metadata resolved a dependency outside the source archive")
        if package["source"] != locked[package["name"], package["version"]]:
            raise ValueError("Frozen metadata resolved a different source than the lock")
        if package["source"] and not path.is_relative_to(tree / "vendor/cargo-sources"):
            raise ValueError("External source was not resolved from the bundled Cargo sources")
        if package["name"] == "zgpui" and path != tree / "vendor/zgpui/Cargo.toml":
            raise ValueError("Frozen metadata did not use the bundled zgpui")
    return actual


def check(archive_path, pkgbuild_path, version, source_pkgrel):
    validate(version)
    source_pkgrel = aur.positive_revision(source_pkgrel)
    checksum = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    expected_recipe = aur.render_recipe("zcall-gui", version, {"source": checksum}, source_pkgrel)
    if pkgbuild_path is not None:
        if pkgbuild_path.read_text() != expected_recipe:
            raise ValueError("Rendered source recipe does not match the archive and current template")
        srcinfo = pkgbuild_path.with_name(".SRCINFO")
        if srcinfo.read_text() != aur.metadata("zcall-gui", version, {"source": checksum}, source_pkgrel):
            raise ValueError("Rendered source metadata does not match the archive and revision")
    with tempfile.TemporaryDirectory(prefix="zcall-gui-aur-source-") as directory:
        work = Path(directory).resolve()
        tree = work / f"zcall-gui-v{version}"
        with tarfile.open(archive_path, "r:gz") as archive:
            members = archive.getmembers()
            seen = set()
            for member in members:
                path = Path(member.name)
                if (path.is_absolute() or ".." in path.parts or not path.parts
                        or path.parts[0] != tree.name or ".git" in path.parts or "target" in path.parts
                        or not (member.isdir() or member.isfile()) or member.name in seen):
                    raise ValueError("Source archive contains an unexpected path, cache, or file type")
                seen.add(member.name)
            archive.extractall(work, filter="data")
        root_lock = tree / "Cargo.lock"
        vendor_lock = tree / "vendor/zgpui/Cargo.lock"
        root_before, vendor_before = lock_graph(root_lock), lock_graph(vendor_lock)
        if tomllib.loads((tree / "Cargo.toml").read_text())["package"]["version"] != version:
            raise ValueError("Source archive version differs")
        recipe = work / "PKGBUILD"
        recipe.write_text(expected_recipe)
        cargo_home = work / "empty-cargo-home"
        cargo_home.mkdir()
        env = os.environ.copy()
        env.update(CARGO_HOME=str(cargo_home), CARGO_NET_OFFLINE="true",
                   CARGO_TARGET_DIR=str(work / "target"), RUSTUP_TOOLCHAIN="1.100.0")
        subprocess.run(
            ["bash", "--noprofile", "--norc", "-c",
             'set -euo pipefail\nsrcdir=$1\nsource "$2"\n'
             '[[ "$pkgname" == zcall-gui && "$pkgver" == "$3" && "$pkgrel" == "$4" ]]\n'
             '[[ ${#sha256sums[@]} == 1 && "${sha256sums[0]}" == "$5" ]]\nprepare\n',
             "aur-source-check", str(work), str(recipe), version, str(source_pkgrel), checksum],
            cwd=work, env=env, check=True, timeout=120,
        )
        if lock_graph(root_lock) != root_before or lock_graph(vendor_lock) != vendor_before:
            raise ValueError("Source prepare changed the locked package versions or dependency graph")
        prepared_locks = (root_lock.read_bytes(), vendor_lock.read_bytes())
        app = metadata(tree, tree / "Cargo.toml", env, root_before)
        library = metadata(tree, tree / "vendor/zgpui/Cargo.toml", env, vendor_before, ("native", "bridge"))
        if prepared_locks != (root_lock.read_bytes(), vendor_lock.read_bytes()):
            raise ValueError("Frozen metadata changed a prepared lockfile")
        if (cargo_home / "git").exists() or (cargo_home / "registry").exists():
            raise ValueError("Source prepare or metadata populated a Git or registry cache")
    print(json.dumps({"source_sha256": checksum, "version": version,
                      "source_pkgrel": source_pkgrel, "cargo_cache_initially_empty": True,
                      "gui_packages": app, "vendor_packages": library,
                      "prepare_and_frozen_metadata": "passed"}, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", default=current())
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--pkgbuild", type=Path)
    parser.add_argument("--source-pkgrel", type=aur.positive_revision, default=1)
    args = parser.parse_args()
    validate(args.version)
    archive = args.archive or ROOT / "_release/dist" / f"zcall-gui-v{args.version}-source.tar.gz"
    check(archive.resolve(strict=True), args.pkgbuild, args.version, args.source_pkgrel)


if __name__ == "__main__":
    main()
