import unittest

from binary_sbom import (
    MissingComponentsError,
    Package,
    filter_sbom,
    npm_runtime_packages,
    parse_cargo_tree,
)


def cargo(name, version):
    purl = f"pkg:cargo/{name}@{version}"
    return {"group": "", "name": name, "version": version, "purl": purl, "bom-ref": purl, "type": "library"}


def npm(name, version, group=""):
    full = f"{group}/{name}" if group else name
    purl = f"pkg:npm/{full}@{version}"
    return {"group": group, "name": name, "version": version, "purl": purl, "bom-ref": purl, "type": "library"}


UI = {**npm("kellnr", "0.1.0"), "type": "application"}


def full_bom():
    return {
        "bomFormat": "CycloneDX",
        "metadata": {"component": UI},
        "components": [
            cargo("kellnr", "7.2.0"),
            cargo("tokio", "1.0.0"),
            cargo("mockall", "0.13.0"),
            npm("vue", "3.5.0"),
            npm("inter", "5.3.0", group="@fontsource-variable"),
            npm("eslint", "9.0.0"),
            {"name": "checkout", "type": "application", "purl": "pkg:github/actions/checkout@v7", "bom-ref": "gh"},
        ],
        "dependencies": [
            {"ref": UI["bom-ref"], "dependsOn": ["pkg:npm/vue@3.5.0", "pkg:npm/eslint@9.0.0"]},
            {"ref": "pkg:cargo/kellnr@7.2.0", "dependsOn": ["pkg:cargo/tokio@1.0.0", "pkg:cargo/mockall@0.13.0"]},
            {"ref": "pkg:cargo/mockall@0.13.0", "dependsOn": []},
        ],
        "annotations": [{"subjects": [UI["bom-ref"]], "text": "generated"}],
    }


CRATES = {Package("kellnr", "7.2.0"), Package("tokio", "1.0.0")}
NPM = {Package("vue", "3.5.0"), Package("@fontsource-variable/inter", "5.3.0")}
ROOT = Package("kellnr", "7.2.0")


class ParseCargoTreeTest(unittest.TestCase):
    def test_parses_name_and_version_and_ignores_suffixes(self):
        output = "\n".join(
            [
                "kellnr v7.2.0 (/repo/crates/kellnr)",
                "tokio v1.0.0",
                "tokio v1.0.0 (*)",
                "serde_derive v1.0.1 (proc-macro)",
                "",
            ]
        )

        self.assertEqual(
            parse_cargo_tree(output),
            {Package("kellnr", "7.2.0"), Package("tokio", "1.0.0"), Package("serde_derive", "1.0.1")},
        )


class NpmRuntimePackagesTest(unittest.TestCase):
    def test_keeps_only_packages_installed_without_dev_and_optional(self):
        lock = {
            "packages": {
                "": {"name": "kellnr", "version": "0.1.0"},
                "node_modules/vue": {"version": "3.5.0"},
                "node_modules/@fontsource-variable/inter": {"version": "5.3.0"},
                "node_modules/vue-router/node_modules/@vue/devtools-api": {"version": "7.0.0"},
                "node_modules/eslint": {"version": "9.0.0", "dev": True},
                "node_modules/fsevents": {"version": "2.3.3", "devOptional": True},
                "node_modules/@esbuild/linux-x64": {"version": "0.28.2", "optional": True},
            }
        }

        self.assertEqual(
            npm_runtime_packages(lock),
            {
                Package("vue", "3.5.0"),
                Package("@fontsource-variable/inter", "5.3.0"),
                Package("@vue/devtools-api", "7.0.0"),
            },
        )


class FilterSbomTest(unittest.TestCase):
    def test_keeps_only_components_compiled_into_the_binary(self):
        bom = filter_sbom(full_bom(), CRATES, NPM, ROOT)

        purls = {c["purl"] for c in bom["components"]}
        self.assertEqual(
            purls,
            {
                "pkg:cargo/tokio@1.0.0",
                "pkg:npm/vue@3.5.0",
                "pkg:npm/@fontsource-variable/inter@5.3.0",
                UI["purl"],
            },
        )

    def test_describes_the_kellnr_crate_and_embeds_the_ui(self):
        bom = filter_sbom(full_bom(), CRATES, NPM, ROOT)

        root = bom["metadata"]["component"]
        self.assertEqual((root["purl"], root["type"]), ("pkg:cargo/kellnr@7.2.0", "application"))
        root_deps = next(d for d in bom["dependencies"] if d["ref"] == root["bom-ref"])
        self.assertEqual(set(root_deps["dependsOn"]), {"pkg:cargo/tokio@1.0.0", UI["bom-ref"]})
        self.assertEqual(bom["annotations"][0]["subjects"], [root["bom-ref"]])

    def test_removes_dropped_components_from_the_dependency_graph(self):
        bom = filter_sbom(full_bom(), CRATES, NPM, ROOT)

        refs = {d["ref"] for d in bom["dependencies"]}
        targets = {t for d in bom["dependencies"] for t in d["dependsOn"]}
        self.assertNotIn("pkg:cargo/mockall@0.13.0", refs | targets)
        self.assertNotIn("pkg:npm/eslint@9.0.0", refs | targets)

    def test_fails_if_a_package_of_the_binary_is_not_in_the_sbom(self):
        crates = CRATES | {Package("openssl-src", "300.5.4+3.5.4")}

        with self.assertRaises(MissingComponentsError) as error:
            filter_sbom(full_bom(), crates, NPM, ROOT)

        self.assertIn("pkg:cargo/openssl-src@300.5.4+3.5.4", str(error.exception))


if __name__ == "__main__":
    unittest.main()
