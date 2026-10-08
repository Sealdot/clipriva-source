#!/usr/bin/env python3
"""Fail new/unreviewed advisory findings, including target-reachable unsoundness."""
import json
from pathlib import Path
import sys


def main():
    folder = Path(sys.argv[1])
    raw = Path(sys.argv[2])
    audit = json.loads((raw / "rust-audit.json").read_text())
    licenses = json.loads((folder / "dependency-licenses.json").read_text())
    reachable = {(p["name"], p["version"]) for p in licenses["rust"]["packages"]}
    assert audit["vulnerabilities"]["count"] == 0 and not audit["vulnerabilities"]["found"]
    # cargo-audit can return success despite registry timeouts: incomplete is not a pass.
    assert "error:" not in (raw / "rust-audit-errors.txt").read_text().lower()
    reviewed = {"RUSTSEC-2024-0370", "RUSTSEC-2025-0081", "RUSTSEC-2025-0075",
                "RUSTSEC-2025-0080", "RUSTSEC-2025-0100", "RUSTSEC-2025-0098"}
    decisions = []
    for kind, findings in audit["warnings"].items():
        for finding in findings:
            advisory = finding["advisory"]["id"]
            package = finding["package"]
            present = (package["name"], package["version"]) in reachable
            if kind == "unmaintained":
                assert advisory in reviewed, advisory
                decision = "Reviewed transitive maintenance risk; no patched version in advisory. Track upstream; frozen source preview only."
            else:
                assert kind == "unsound" and advisory == "RUSTSEC-2024-0429" and not present, advisory
                decision = "glib 0.18.5 is absent from the macOS target graph; unsupported Linux GTK graph only. Linux use requires remediation."
            decisions.append({"advisory": advisory, "package": package["name"], "version": package["version"],
                              "kind": kind, "reachable_on_runner_target": present, "decision": decision})
    result = {"commit": licenses["commit"], "target": licenses["target"], "database": audit["database"],
              "vulnerabilities": 0, "warnings": decisions, "ignored_advisories": [], "complete": True}
    (folder / "rust-advisory-review.json").write_text(json.dumps(result, indent=2) + "\n")
    print("RustSec: zero vulnerability entries;", len(decisions), "explicitly reviewed warnings; no ignored advisories")


if __name__ == "__main__":
    main()
