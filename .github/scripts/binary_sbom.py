"""Reduce the CycloneDX SBOM of the repository to the kellnr binary of a platform.

The SBOM of `cdxgen` describes the whole repository, including test crates,
dev tooling and CI actions. This script keeps only the crates compiled into
the release binaries of one platform (normal and build dependencies) and
the npm packages of the embedded web UI that npm installs without dev and
optional dependencies.

Usage: binary_sbom.py <linux|windows|macos> <repository SBOM> <output SBOM>
"""

import copy
import json
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

REPO_ROOT = Path(__file__).resolve().parents[2]
UI_LOCKFILE = REPO_ROOT / "ui" / "package-lock.json"
ROOT_CRATE = "kellnr"


class Platform(NamedTuple):
    targets: tuple[str, ...]
    cargo_features: tuple[str, ...]


# Targets and feature flags of the release builds in .github/workflows/release.yaml.
PLATFORMS = {
    "linux": Platform(
        targets=(
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-gnu",
            "aarch64-unknown-linux-musl",
        ),
        cargo_features=("--features", "vendored-openssl"),
    ),
    "windows": Platform(
        targets=("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"),
        cargo_features=("--no-default-features",),
    ),
    "macos": Platform(targets=("aarch64-apple-darwin",), cargo_features=()),
}


class Package(NamedTuple):
    name: str
    version: str


class MissingComponentsError(Exception):
    pass


def parse_cargo_tree(output: str) -> set[Package]:
    packages = set()
    for line in output.splitlines():
        if not line.strip():
            continue
        name, version = line.split()[:2]
        packages.add(Package(name, version.removeprefix("v")))
    return packages


def npm_runtime_packages(lock: dict) -> set[Package]:
    return {
        Package(path.rsplit("node_modules/", 1)[-1], entry["version"])
        for path, entry in lock["packages"].items()
        if path and not any(entry.get(flag) for flag in ("dev", "devOptional", "optional"))
    }


def cargo_tree_args(platform: Platform, target: str) -> list[str]:
    return [
        "cargo", "tree", "--quiet",
        "--package", ROOT_CRATE,
        *platform.cargo_features,
        "--target", target,
        "--edges", "normal,build",
        "--prefix", "none",
        "--format", "{p}",
    ]


def crates_of_release_binaries(platform: Platform) -> set[Package]:
    crates = set()
    for target in platform.targets:
        output = subprocess.run(
            cargo_tree_args(platform, target),
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        crates |= parse_cargo_tree(output)
    return crates


def component_package(component: dict) -> tuple[str, Package] | None:
    purl = component.get("purl", "")
    for ecosystem in ("cargo", "npm"):
        if purl.startswith(f"pkg:{ecosystem}/"):
            group = component.get("group")
            name = f"{group}/{component['name']}" if group else component["name"]
            return ecosystem, Package(name, component["version"])
    return None


def filter_sbom(bom: dict, crates: set[Package], npm: set[Package], root: Package) -> dict:
    wanted = {("cargo", p) for p in crates} | {("npm", p) for p in npm}
    kept = [c for c in bom["components"] if component_package(c) in wanted]

    missing = wanted - {component_package(c) for c in kept}
    if missing:
        purls = sorted(f"pkg:{ecosystem}/{p.name}@{p.version}" for ecosystem, p in missing)
        raise MissingComponentsError("Packages of the binary missing in the SBOM: " + ", ".join(purls))

    root_component = next(c for c in kept if component_package(c) == ("cargo", root))
    ui_component = bom["metadata"]["component"]

    result = copy.deepcopy(bom)
    result["metadata"]["component"] = {**root_component, "type": "application"}
    result["components"] = [c for c in kept if c is not root_component] + [ui_component]
    result["dependencies"] = filter_dependencies(bom["dependencies"], kept + [ui_component], root_component, ui_component)
    for annotation in result.get("annotations", []):
        annotation["subjects"] = [
            root_component["bom-ref"] if s == ui_component["bom-ref"] else s for s in annotation["subjects"]
        ]
    return result


def filter_dependencies(dependencies: list[dict], components: list[dict], root: dict, ui: dict) -> list[dict]:
    refs = {c["bom-ref"] for c in components}
    filtered = []
    for dependency in dependencies:
        if dependency["ref"] not in refs:
            continue
        depends_on = [r for r in dependency.get("dependsOn", []) if r in refs]
        if dependency["ref"] == root["bom-ref"]:
            depends_on.append(ui["bom-ref"])
        filtered.append({**dependency, "dependsOn": depends_on})
    return filtered


def main() -> None:
    if len(sys.argv) != 4 or sys.argv[1] not in PLATFORMS:
        sys.exit(__doc__)
    platform = PLATFORMS[sys.argv[1]]
    source, output = map(Path, sys.argv[2:])

    crates = crates_of_release_binaries(platform)
    root = next(p for p in crates if p.name == ROOT_CRATE)
    npm = npm_runtime_packages(json.loads(UI_LOCKFILE.read_text()))
    bom = filter_sbom(json.loads(source.read_text()), crates, npm, root)

    output.write_text(json.dumps(bom, indent=2) + "\n")
    print(f"{output}: {len(bom['components'])} components ({len(crates)} crates, {len(npm)} npm packages, web UI)")


if __name__ == "__main__":
    main()
