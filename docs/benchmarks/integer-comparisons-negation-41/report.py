"""Derive complete numerical tables; no new measurements or historical edits."""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent

def load(relative):
    return json.loads((HERE / relative).read_text(encoding="utf-8"))

def pair(values):
    return " / ".join("verify-only" if v is None else f"{v:g}" for v in values)

def report():
    main = load("collection-1/summary.json")
    supplement = load("expressions-2/summary.json")
    historical = {r["name"]: r for r in load("../integer-locals-40/collection-1/summary.json")["workloads"]}
    stats = {}
    lines = ["# Derived #41 results", "", "Current None versus current Basic; cumulative #39+#40+#41, not isolated #41 timing.", "Pairs are repetitions 1 / 2; compiler µs/compilation, guest ns/main call.", ""]
    for title, data in (("Unchanged historical corpus", main), ("Expression-only supplement (balanced trees)", supplement)):
        lines += ["## " + title, "", "| Workload | Bytes None → Basic | Source None | Source Basic | Typed None | Typed Basic | Guest None | Guest Basic |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
        names = []
        changes = {s: {"higher_both": [], "lower_both": [], "mixed_or_equal": []} for s in ("source_compile", "typed_compile", "guest")}
        sums = {level: sum(r["levels"][level]["moduleBytes"] for r in data["workloads"]) for level in ("None", "Basic")}
        for r in data["workloads"]:
            n, b = (r["levels"][l] for l in ("None", "Basic"))
            cells = [r["name"], f"{n['moduleBytes']} → {b['moduleBytes']}"]
            for stage in ("source_compile", "typed_compile"):
                cells += [pair([v["median_us"] for v in d["compiler"][stage]]) for d in (n, b)]
            cells += [pair(d["guest_medians_ns"]) for d in (n, b)]
            lines.append("| " + " | ".join(cells) + " |")
            deltas = {}
            for stage in changes:
                a, z = ([d["guest_medians_ns"] for d in (n, b)] if stage == "guest" else
                    [[v["median_us"] for v in d["compiler"][stage]] for d in (n, b)])
                if None in a + z:
                    continue
                p = [(y / x - 1) * 100 for x, y in zip(a, z)]
                deltas[stage] = p
                category = "higher_both" if all(v > 0 for v in p) else "lower_both" if all(v < 0 for v in p) else "mixed_or_equal"
                changes[stage][category].append(r["name"])
            names.append({"name": r["name"], "percent_change": deltas})
        stats[title] = {"module_sums": sums, "module_percent_change": (sums['Basic']/sums['None']-1)*100,
            "directions": changes, "per_case": names}
        lines += ["", f"Synthetic unweighted module sum: {sums['None']} → {sums['Basic']} bytes ({stats[title]['module_percent_change']:.2f}%).", ""]
    lines += ["## Historical #40 Basic → current Basic size only", "", "Identical historical corpus sources required; historical timing is NOT a matched incremental control.", "", "| Case | #40 Basic bytes | Current Basic bytes | Delta |", "| --- | ---: | ---: | ---: |"]
    total = 0
    differences = []
    for row in main["workloads"]:
        old = historical[row["name"]]["levels"]["Basic"]
        new = row["levels"]["Basic"]
        assert old["sourceSha256"] == new["sourceSha256"]
        delta = new["moduleBytes"] - old["moduleBytes"]
        total += delta
        if delta:
            lines.append(f"| {row['name']} | {old['moduleBytes']} | {new['moduleBytes']} | {delta:+} |")
            differences.append({"name": row["name"], "old_bytes": old['moduleBytes'], "new_bytes": new['moduleBytes'], "delta_bytes": delta})
    stats["incremental_size_vs_40"] = {"delta_bytes": total, "changed_sizes": differences}
    lines += ["", f"Net historical size change: {total:+} bytes. Cases absent from the table have unchanged byte lengths (not necessarily identical hashes).", ""]
    (HERE / "results.md").write_text("\n".join(lines), encoding="utf-8")
    (HERE / "results.json").write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(stats, indent=2))

if __name__ == "__main__":
    report()
