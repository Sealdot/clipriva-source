#!/usr/bin/env python3
"""Install only exact, checksum-verified Release source into the clean CI checkout."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    root = Path.cwd()
    assert not run("git", "status", "--porcelain=v1", "--untracked-files=all")
    folder = Path(os.environ["RUNNER_TEMP"]) / "clipriva-release-download"
    folder.mkdir()
    run("gh", "release", "download", os.environ["RELEASE_TAG"], "--repo", os.environ["GITHUB_REPOSITORY"],
        "--dir", str(folder), "--pattern", "ClipRiva-source.tar.gz", "--pattern", "SHA256SUMS", "--pattern", "SOURCE_RECORD.json")
    sums = {}
    for line in (folder / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        sums[name.lstrip("*")] = digest
    for name in ["ClipRiva-source.tar.gz", "SOURCE_RECORD.json"]:
        assert hashlib.sha256((folder / name).read_bytes()).hexdigest() == sums[name], name
    record = json.loads((folder / "SOURCE_RECORD.json").read_text())
    assert record["commit"] == os.environ["GITHUB_SHA"] == run("git", "rev-parse", "HEAD")
    assert record["tag"] == os.environ["RELEASE_TAG"]
    tracked = set(run("git", "ls-files").splitlines())
    stage = folder / "extracted"
    stage.mkdir()
    with tarfile.open(folder / "ClipRiva-source.tar.gz") as archive:
        files = {}
        for member in archive.getmembers():
            parts = Path(member.name).parts
            assert parts[0] == "ClipRiva-source" and ".." not in parts and not Path(member.name).is_absolute()
            if member.isdir():
                continue
            assert member.isfile(), member.name
            relative = str(Path(*parts[1:]))
            assert relative not in files
            files[relative] = member
        assert set(files) == tracked
        for relative, member in files.items():
            data = archive.extractfile(member).read()
            assert data == (root / relative).read_bytes(), relative
            mode = int(run("git", "ls-files", "--stage", "--", relative).split()[0], 8)
            assert bool(member.mode & 0o111) == bool(mode & 0o111), relative
            destination = stage / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            destination.chmod(member.mode)
    for relative in files:
        shutil.copy2(stage / relative, root / relative)
    assert not run("git", "status", "--porcelain=v1", "--untracked-files=all")
    evidence = Path(os.environ["RUNNER_TEMP"]) / "clipriva-source-evidence"
    evidence.mkdir(exist_ok=True)
    report = {"commit": record["commit"], "tag": record["tag"], "files": len(files),
              "archive_sha256": sums["ClipRiva-source.tar.gz"], "verified": True,
              "source": "Release download; every source byte and executable mode matched the committed checkout before replacement; subsequent install/tests use downloaded files."}
    (evidence / "release-download-verification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
