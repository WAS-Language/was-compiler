"""Losslessly package one raw Git diff; append-only manifests, no index changes.

Run from repository root with Python 3: python -B docs/benchmarks/integer-comparisons-negation-41/package_evidence.py
Existing output files are refused; original metadata/audits/manifests are never rewritten.
"""
import datetime
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RAW = "collection-1/git-diff-before.txt"
PACKED = RAW + ".gz"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def encode(raw):
    output = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=output, compresslevel=9, mtime=0) as stream:
        stream.write(raw)
    return output.getvalue()


def main():
    if Path.cwd().resolve() != ROOT:
        raise RuntimeError("Run from repository root")
    for name in (PACKED, "packaging-reconciliation.json", "final-commit-artifact-hashes.json"):
        if (HERE / name).exists():
            raise RuntimeError("Refusing existing output: " + name)
    manifests = {}
    manifest_bytes = {}
    for name in ("artifact-hashes.json", "final-integration-artifact-hashes.json"):
        manifest_bytes[name] = (HERE / name).read_bytes()
        manifests[name] = json.loads(manifest_bytes[name])
    raw = (HERE / RAW).read_bytes()
    raw_identity = {"bytes": len(raw), "sha256": digest(raw)}
    for manifest in manifests.values():
        assert manifest[RAW] == raw_identity, "Raw snapshot differs from historical manifest"
    comparisons = []
    for name, manifest in manifests.items():
        changed = []
        for relative, identity in manifest.items():
            if relative == RAW:
                continue
            data = (HERE / relative).read_bytes()
            current = {"bytes": len(data), "sha256": digest(data)}
            if current != identity:
                changed.append({"path": relative, "historical": identity, "current": current})
        assert [row["path"] for row in changed] == ["README.md"], "Unexpected retained evidence changes"
        comparisons.append({"manifest": name, "manifest_sha256": digest(manifest_bytes[name]),
            "retained_artifact_hash_changes": changed,
            "separately_repackaged_original": RAW})
    packed = encode(raw)
    assert gzip.decompress(packed) == raw
    assert encode(raw) == packed, "Gzip is not deterministic"
    with (HERE / PACKED).open("xb") as stream:
        stream.write(packed)
    restored = gzip.decompress((HERE / PACKED).read_bytes())
    assert restored == raw and digest(restored) == raw_identity["sha256"]
    # Only remove the plain file after the stored gzip verifies byte-for-byte.
    (HERE / RAW).unlink()
    assert not (HERE / RAW).exists()
    for name, data in manifest_bytes.items():
        assert (HERE / name).read_bytes() == data, "Historical manifest changed"
    broken_links = []
    for link in re.findall(r"\]\(([^)]+)\)", (HERE / "README.md").read_text(encoding="utf-8")):
        if link in ("packaging-reconciliation.json", "final-commit-artifact-hashes.json"):
            continue
        if not link.startswith(("https:", "http:", "#")) and not (HERE / link.split('#')[0]).exists():
            broken_links.append(link)
    assert not broken_links, broken_links
    command = ["git", "--no-pager", "diff", "--check"]
    check = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=30, check=False)
    assert check.returncode == 0, check.stdout + check.stderr
    reconciliation = {
        "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "command": sys.argv, "python": sys.version,
        "operation": "Lossless deterministic gzip packaging, not raw-output rewriting or a timing rerun",
        "original_decompressed_path": RAW, "packaged_path": PACKED,
        "original_raw": raw_identity,
        "gzip": {"bytes": len(packed), "sha256": digest(packed), "mtime": 0,
            "embedded_filename": "", "compression_level": 9},
        "decompressed_identity": {"bytes": len(restored), "sha256": digest(restored)},
        "stored_gzip_verified_before_plain_removal": True,
        "deterministic_recompression_byte_identical": True,
        "space_only_raw_context_lines_preserved": sum(line == b" " for line in raw.splitlines()),
        "original_metadata_audits_manifests_unchanged": True,
        "historical_manifest_comparisons": comparisons,
        "historical_paths_and_digests_apply_to": "Original decompressed bytes; old manifests are historical snapshots",
        "readme_links_checked": True,
        "unstaged_diff_check": {"command": command, "exit_code": check.returncode,
            "stdout": check.stdout.decode("utf-8", errors="replace"), "stderr": check.stderr.decode("utf-8", errors="replace")},
        "index_modified": False,
        "staged_check_caution": "Cached diff still contains old staged text until integrator stages packaging changes",
        "owner_reported_latest_validation_not_rerun": "1,287 tests across 81 targets passed; review no hard findings, one low duplication suggestion deferred",
        "timings_unchanged": True,
    }
    write_json(HERE / "packaging-reconciliation.json", reconciliation)
    artifacts = {p.relative_to(HERE).as_posix(): {"bytes": p.stat().st_size, "sha256": digest(p.read_bytes())}
        for p in sorted(HERE.rglob('*')) if p.is_file() and p.name != "final-commit-artifact-hashes.json"}
    write_json(HERE / "final-commit-artifact-hashes.json", artifacts)
    print(json.dumps({"original_raw": raw_identity, "gzip": reconciliation["gzip"],
        "space_only_context_lines": reconciliation["space_only_raw_context_lines_preserved"],
        "retained_original_hash_changes": ["README.md"], "artifacts": len(artifacts),
        "unstaged_diff_check_passed": True, "index_modified": False}, indent=2))


if __name__ == "__main__":
    main()
