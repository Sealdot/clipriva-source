#!/usr/bin/env python3
"""Write portable target license evidence from successful pnpm/Cargo metadata."""

import argparse
import hashlib
import json
import re
from pathlib import Path


def restricted_identifier(value):
    return bool(re.search(r"(?:A?GPL|LGPL|SSPL|BUSL|LicenseRef|Commons.?Clause|non.?commercial|CC-BY-NC)", value, re.I))


def restriction(expression):
    """An OR alternative may be selected; every AND obligation must be considered."""
    tokens = re.findall(r"\(|\)|[A-Za-z0-9.+-]+", expression)
    if re.sub(r"\s+", "", expression) != "".join(tokens):
        raise ValueError("unsupported license expression")
    cursor = 0

    def atom():
        nonlocal cursor
        if cursor >= len(tokens):
            raise ValueError("incomplete license expression")
        token = tokens[cursor]
        cursor += 1
        if token == "(":
            value = alternatives()
            if cursor >= len(tokens) or tokens[cursor] != ")":
                raise ValueError("unbalanced license expression")
            cursor += 1
            return value
        if token in {"AND", "OR", "WITH", ")"}:
            raise ValueError("invalid license identifier")
        value = restricted_identifier(token)
        if cursor < len(tokens) and tokens[cursor] == "WITH":
            cursor += 1
            if cursor >= len(tokens) or tokens[cursor] in {"AND", "OR", "WITH", "(", ")"}:
                raise ValueError("invalid license exception")
            cursor += 1
        return value

    def obligations():
        nonlocal cursor
        value = atom()
        while cursor < len(tokens) and tokens[cursor] == "AND":
            cursor += 1
            other = atom()
            value = value or other
        return value

    def alternatives():
        nonlocal cursor
        value = obligations()
        while cursor < len(tokens) and tokens[cursor] == "OR":
            cursor += 1
            other = obligations()
            value = value and other
        return value

    value = alternatives()
    if cursor != len(tokens):
        raise ValueError("trailing license expression tokens")
    return value


def node_components(metadata):
    rows = {}
    for license_name, packages in metadata.items():
        if not isinstance(packages, list):
            raise ValueError("unexpected pnpm license metadata shape")
        for package in packages:
            name = package["name"]
            versions = package.get("versions") or [package["version"]]
            for version in versions:
                rows[(name, version)] = {"name": name, "version": version, "license": license_name}
    if not rows:
        raise ValueError("empty pnpm dependency metadata")
    return sorted(rows.values(), key=lambda row: (row["name"], row["version"]))


def cargo_components(metadata):
    graph = metadata["resolve"]
    nodes = {node["id"]: node for node in graph["nodes"]}
    pending = [graph["root"]]
    reached = set()
    while pending:
        identifier = pending.pop()
        if identifier in reached:
            continue
        reached.add(identifier)
        pending.extend(nodes[identifier]["dependencies"])
    rows = []
    for package in metadata["packages"]:
        if package["id"] not in reached or not (package.get("source") or "").startswith("registry+"):
            continue
        rows.append({"name": package["name"], "version": package["version"], "license": package.get("license")})
    if not rows:
        raise ValueError("empty Cargo target dependency graph")
    return sorted(rows, key=lambda row: (row["name"], row["version"]))


def review(rows):
    missing = []
    restricted = []
    alternatives = []
    unsupported = []
    legacy = []
    for row in rows:
        label = row["name"] + "@" + row["version"]
        value = row["license"]
        if not value or value.lower() in {"unknown", "unlicensed", "none"}:
            missing.append(label)
            continue
        # Cargo retained slash-separated alternatives from its pre-SPDX syntax.
        # Normalize only a pure list of identifiers; retain the original declaration.
        # https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields
        if re.fullmatch(r"[A-Za-z0-9.+-]+(?:\s*/\s*[A-Za-z0-9.+-]+)+", value):
            value = re.sub(r"\s*/\s*", " OR ", value)
            row["normalized_license"] = value
            legacy.append(label)
        try:
            if restriction(value):
                restricted.append(label)
            elif restricted_identifier(value):
                alternatives.append(label)
        except ValueError:
            unsupported.append(label)
    return {"components": len(rows), "missing": missing, "restricted_without_unrestricted_alternative": restricted,
            "restricted_mentions_with_alternative": alternatives, "unsupported_expressions": unsupported,
            "legacy_syntax_normalizations": legacy, "packages": rows}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--node-metadata", type=Path, required=True)
    parser.add_argument("--cargo-metadata", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    node = review(node_components(json.loads(args.node_metadata.read_text())))
    cargo = review(cargo_components(json.loads(args.cargo_metadata.read_text())))
    args.output.mkdir(parents=True, exist_ok=True)
    result = {"commit": args.commit, "target": args.target, "cargo_features": "all", "scope":
              "Current-platform pnpm installed metadata and reachable Cargo host-target graph, including build/test dependencies; not all-platform or a binary notice audit.",
              "lockfile_sha256": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in
                                  [Path("pnpm-lock.yaml"), Path("src-tauri/Cargo.lock")]}, "node": node, "rust": cargo}
    (args.output / "dependency-licenses.json").write_text(json.dumps(result, indent=2) + "\n")
    for ecosystem, summary in [("Node", node), ("Rust", cargo)]:
        print(ecosystem, "components:", summary["components"], "missing:", len(summary["missing"]),
              "restricted:", len(summary["restricted_without_unrestricted_alternative"]),
              "unsupported:", len(summary["unsupported_expressions"]))
    if any(summary[field] for summary in [node, cargo] for field in
           ["missing", "restricted_without_unrestricted_alternative", "unsupported_expressions"]):
        raise SystemExit("Dependency license findings require review; evidence is not a pass.")


if __name__ == "__main__":
    main()
