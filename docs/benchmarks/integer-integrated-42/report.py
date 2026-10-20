"""Derive #42 tables from immutable records; no builds, tests or measurements.

Run from any directory with Python 3 (standard library only). Only results.json
and results.md beside this script are written. Historical timings are not controls.
"""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
IDENTITY = ("sourceBytes", "moduleBytes", "sourceSha256", "moduleSha256")
STAGES = ("source_compile", "typed_compile", "guest")


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def identity(row):
    return {key: row[key] for key in IDENTITY}


def percent(before, after):
    return (after / before - 1) * 100


def pair(values, percentage=False):
    return " / ".join(
        "verify-only" if value is None else
        f"{value:+.2f}%" if percentage else f"{value:g}"
        for value in values
    )


def main():
    collection = HERE / "collection-1"
    paths = {
        "current": collection / "summary.json",
        "historical_38": HERE.parent / "integer-specialization/summary.json",
        "historical_41": HERE.parent / "integer-comparisons-negation-41/collection-1/summary.json",
    }
    current, old38, old41 = (load(paths[key]) for key in paths)
    baseline = {row["name"]: row for row in old38["workloads"]}
    previous = {row["name"]: row for row in old41["workloads"]}
    assert len(current["workloads"]) == len(baseline) == len(previous) == 27
    assert {row["name"] for row in current["workloads"]} == set(baseline) == set(previous)
    assert current["verify_records"] == 108 and current["enabled_presets_identical"]
    assert load(collection / "stability.json")["inputs_unchanged"]
    assert load(collection / "hashes-before.json") == load(collection / "hashes-after.json")
    revision = (collection / "git-head.txt").read_text().strip()
    assert revision == "0e1b5225a6d63c18244ab8c06fc460dd2d7a0020"
    directions = {stage: {key: [] for key in (
        "higher_both", "lower_both", "mixed_or_equal", "verify_only"
    )} for stage in STAGES}
    cases = []
    for row in current["workloads"]:
        name = row["name"]
        none, basic = (row["levels"][level] for level in ("None", "Basic"))
        historical = {
            "38_None": identity(baseline[name]),
            "41_None": identity(previous[name]["levels"]["None"]),
            "41_Basic": identity(previous[name]["levels"]["Basic"]),
        }
        matches = {
            "None_vs_38": identity(none) == historical["38_None"],
            "None_vs_41": identity(none) == historical["41_None"],
            "Basic_vs_41": identity(basic) == historical["41_Basic"],
            "source_None_vs_Basic": all(none[k] == basic[k] for k in ("sourceBytes", "sourceSha256")),
        }
        assert all(matches.values()), (name, matches)
        timing = {}
        for stage in STAGES:
            values = [level["guest_medians_ns"] if stage == "guest" else
                      [run["median_us"] for run in level["compiler"][stage]]
                      for level in (none, basic)]
            assert all(len(value) == 2 for value in values)
            a, b = values
            if any(value is None for value in a + b):
                assert name == "trap_order" and stage == "guest" and a == b == [None, None]
                deltas, absolute, direction = [None, None], [None, None], "verify_only"
            else:
                deltas = [percent(x, y) for x, y in zip(a, b)]
                absolute = [y - x for x, y in zip(a, b)]
                direction = ("higher_both" if all(v > 0 for v in deltas) else
                             "lower_both" if all(v < 0 for v in deltas) else "mixed_or_equal")
            directions[stage][direction].append(name)
            timing[stage] = {
                "None": a, "Basic": b, "delta": absolute,
                "percent_change": deltas, "direction": direction,
            }
        cases.append({
            "name": name, "levels": row["levels"], "timing": timing,
            "module_delta_bytes": basic["moduleBytes"] - none["moduleBytes"],
            "module_percent_change": percent(none["moduleBytes"], basic["moduleBytes"]),
            "historical_identity": historical, "identity_matches": matches,
        })
    totals = {level: sum(row["levels"][level]["moduleBytes"] for row in cases)
              for level in ("None", "Basic")}
    totals.update({"delta_bytes": totals["Basic"] - totals["None"],
                   "percent_change": percent(totals["None"], totals["Basic"]),
                   "smaller": sum(row["module_delta_bytes"] < 0 for row in cases),
                   "equal": sum(row["module_delta_bytes"] == 0 for row in cases),
                   "larger": sum(row["module_delta_bytes"] > 0 for row in cases)})
    files = sorted(collection.iterdir()) + [HERE / name for name in (
        "launcher.json", "collect.py", "baseline-self-test.json", "baseline-self-test.txt",
        "matched-self-test.json", "matched-self-test.txt",
    )] + list(paths.values())[1:]
    hashes = {path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in files if path.is_file()}
    commands = {path.name: load(path) for path in sorted(collection.glob("*.json"))
                if isinstance(load(path), dict) and "exit_code" in load(path)}
    result = {
        "scope": "Current cumulative #39/#40/#41 None vs Basic; no isolated incremental timing claim",
        "release_and_regression_acceptance": "PENDING; experimental; None remains default",
        "revision": revision, "environment": load(collection / "environment.json"),
        "launcher": load(HERE / "launcher.json"), "machines": current["machines"],
        "stability": load(collection / "stability.json"),
        "recorded_input_hashes": load(collection / "hashes-before.json"),
        "oracle_pairs": 108, "identity_match_counts": {key: sum(row["identity_matches"][key] for row in cases)
                                                     for key in cases[0]["identity_matches"]},
        "size_totals": totals, "directions": directions,
        "direction_counts": {stage: {key: len(names) for key, names in groups.items()}
                             for stage, groups in directions.items()},
        "recorded_commands": commands, "input_sha256": hashes, "cases": cases,
    }
    lines = ["# Derived integrated integer results (#42)", "",
             "Current cumulative #39/#40/#41: None versus Basic, not isolated incremental pass timing.",
             "Release/regression acceptance **PENDING**; experimental, None default.", "",
             "Derived by [report.py](report.py) from immutable [collection-1/summary.json](collection-1/summary.json).",
             "All pairs are repetition 1 / 2, paired by repetition (not adjacent command order).",
             "Delta = Basic − None; percent = 100 × (Basic / None − 1). Positive time means slower.",
             "Strict sign in both repeats gives higher_both/lower_both; zeros or reversals give mixed_or_equal.",
             "These classifications are descriptive, not significance tests or accepted regressions.", "",
             "## Size (all 27 independent modules)", "",
             f"Unweighted sum: **{totals['None']:,} → {totals['Basic']:,} bytes** "
             f"({totals['delta_bytes']:+,}; {totals['percent_change']:+.2f}%). "
             f"{totals['smaller']} shrink, {totals['equal']} unchanged, {totals['larger']} grow.",
             "Synthetic corpus sum, not a combined application or production-traffic estimate.", "",
             "| Case | None bytes | Basic bytes | Delta bytes | Delta % |",
             "| --- | ---: | ---: | ---: | ---: |"]
    for row in cases:
        lines.append(f"| {row['name']} | {row['levels']['None']['moduleBytes']} | "
                     f"{row['levels']['Basic']['moduleBytes']} | {row['module_delta_bytes']:+} | "
                     f"{row['module_percent_change']:+.2f}% |")
    for stage in STAGES:
        unit = "ns/main call" if stage == "guest" else "µs/compilation"
        lines += ["", f"## {stage} ({unit})", "",
                  "| Case | None 1 / 2 | Basic 1 / 2 | Delta 1 / 2 | Delta % 1 / 2 | Direction |",
                  "| --- | ---: | ---: | ---: | ---: | --- |"]
        for row in cases:
            t = row["timing"][stage]
            lines.append("| " + " | ".join([row["name"], pair(t["None"]), pair(t["Basic"]),
                         pair(t["delta"]), pair(t["percent_change"], True), t["direction"]]) + " |")
        lines += ["", "; ".join(f"{key}: {len(names)}" for key, names in directions[stage].items()) + "."]
    lines += ["", "## Exact historical identities", "",
              "All 27 current None source/module identities equal #38 and #41 None; all 27 current Basic",
              "source/module identities equal #41 Basic, including byte lengths and full SHA-256 hashes.",
              "Current None/Basic source identities also match in all 27 cases. Thus size delta versus #41",
              "is exactly zero at both levels. Historical timing is not used as a matched control.",
              "Full historical and current identities are retained per case in [results.json](results.json).", "",
              "| Case | Source SHA-256 (all compared records) | None module SHA-256 (#38/#41/current) | Basic module SHA-256 (#41/current) |",
              "| --- | --- | --- | --- |"]
    for row in cases:
        n, b = (row["levels"][level] for level in ("None", "Basic"))
        lines.append(f"| {row['name']} | `{n['sourceSha256']}` | `{n['moduleSha256']}` | `{b['moduleSha256']}` |")
    lines += ["", "## Provenance and precision", "",
              f"Measured base HEAD: `{revision}` plus the recorded dirty test-only diff; see README.",
              "Compiler values retain the recorder's parsed Divan min/max/median/mean aggregates in results.json;",
              "100 samples × one compilation, OS timer. Raw per-compilation samples were not recorded.",
              "Guest logs retain 20 raw ns/main-call samples (64 calls/sample), after 128 warmup calls;",
              "trap_order is verify-only. Neither raw guest samples nor Divan aggregates are interchangeable.",
              "See [README](README.md) for boundaries, limitations, commands and conversion tradeoffs.",
              "results.json includes all recorded command metadata, environment, source hashes and evidence-file hashes.",
              "No new validation runs or release claims are made by this derivation.", ""]
    (HERE / "results.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (HERE / "results.md").write_text("\n".join(lines), encoding="utf-8")
    print(json.dumps({key: result[key] for key in (
        "size_totals", "direction_counts", "identity_match_counts"
    )}, indent=2))
    for row in cases:
        if row["name"] in ("literal_32", "locals_32", "compare_32", "negate_32", "loop_local", "negative_zero", "negate_zero", "positive_zero"):
            print(row["name"], json.dumps({stage: row["timing"][stage]["percent_change"] for stage in STAGES}))


if __name__ == "__main__":
    main()
