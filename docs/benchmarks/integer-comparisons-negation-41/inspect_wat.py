"""Post-timing inspection through the existing CLI/wasmprinter, not production facts.

Exact corpus-source and CLI-binary identities are checked against collection-1.
Counts cover only function index 1 (main), excluding parameters and helper bodies.
WAT instruction counts are static occurrences, not dynamic execution counts.
"""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location(
    "matched", ROOT / "docs/benchmarks/integer-add-sub-39/record_matched.py")
matched = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matched)
base = matched.baseline


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    out = HERE / "inspection"
    out.mkdir(exist_ok=False)
    reference = json.loads((HERE / "collection-1/summary.json").read_text())
    reference = {r["name"]: r for r in reference["workloads"]}
    timed_hashes = json.loads((HERE / "collection-1/hashes-after.json").read_text())
    start = matched.hashes()
    base.write_json(out / "hashes-before.json", start)
    base.require(start == timed_hashes, "Inputs changed since timing; do not conflate evidence")
    env = dict(os.environ, CARGO_NET_OFFLINE="true")
    rows = []
    completed = False
    try:
        base.record(out, "cli-build", ["cargo", "build", "--offline", "-p", "was-compiler-cli"], env)
        binary = ROOT / "target/debug/was-compiler.exe"
        base.write_json(out / "environment.json", {
            "utc": base.utc(), "python": sys.version, "script_sha256": digest(Path(__file__).read_bytes()),
            "cli_path": str(binary), "cli_sha256": digest(binary.read_bytes()),
            "scope": "main index 1 only, local declarations exclude parameters; static instruction occurrences",
        })
        sources = {}
        for family, statement in (
            ("locals", "{ let a = 120; let b = a + 30; let c = b - 7; total += c; };"),
            ("compare", "{ let a = 120 + 30; if a > 149 { total += 1; } if a == 150 { total += 2; } if a <= 149 { total += 100; } else { total += 0; } };"),
            ("negate", "{ let a = 120 + 30; let b = -a; total += b + 7; };"),
        ):
            sources[family + "_32"] = "fn main(x: number) -> number { let total = 0;\n" + (statement + "\n") * 32 + "return total; }"
        for name, body in (
            ("loop_local", "let total = 0; for i in 0..32 { let a = 120; let b = a + 23; total += b; } return total;"),
            ("mutable", "let a = 1; a = 2.5; return a + 1;"),
            ("capture", "let a = 120; let f = () => a + 23; a = 121; return f();"),
            ("negate_zero", "let z = 0; return -z;"),
            ("negate_min", "let a = -2147483648; return -a;"),
        ):
            sources[name] = f"fn main(x: number) -> number {{ {body} }}"
        for name, source in sources.items():
            data = source.encode()
            base.require(digest(data) == reference[name]["levels"]["None"]["sourceSha256"],
                         "Inspection source does not match corpus")
            source_path = out / (name + ".was")
            source_path.write_bytes(data)
            for level, flag in (("None", "-O0"), ("Basic", "-O1")):
                stem = name + "-" + level
                command = [str(binary), str(source_path), flag, "--host", "mark=env.mark(number)->number"]
                wasm = out / (stem + ".wasm")
                wat = out / (stem + ".wat")
                base.record(out, stem + "-binary", command + ["-o", str(wasm)], env)
                base.record(out, stem + "-wat", command + ["-o", str(wat)], env)
                module = wasm.read_bytes()
                expected = reference[name]["levels"][level]
                base.require(digest(module) == expected["moduleSha256"] and len(module) == expected["moduleBytes"],
                             "CLI binary differs from timed module")
                text = wat.read_text(encoding="utf-8")
                functions = re.split(r"(?m)^  (?=\(func )", text)[1:]
                body = next(f for f in functions if f.startswith("(func (;1;)"))
                body = re.split(r"(?m)^  \(", body, maxsplit=1)[0]
                locals_ = re.findall(r"\(local ([^)]+)\)", body)
                types = " ".join(locals_).split()
                base.require(set(types) <= {"i32", "i64", "f64", "f32"}, "Unexpected WAT locals syntax")
                ops = re.findall(r"(?m)^\s+([a-z][a-z0-9_.]*)\b", body)
                counts = {op: ops.count(op) for op in (
                    "f64.convert_i32_s", "f64.convert_i32_u", "i32.trunc_f64_s", "i32.trunc_f64_u",
                    "i32.add", "i32.sub", "i32.gt_s", "i32.eq", "i32.le_s",
                    "f64.add", "f64.sub", "f64.neg", "f64.gt", "f64.eq", "f64.le", "f64.store")}
                rows.append({"name": name, "level": level, "sourceSha256": digest(data),
                             "moduleSha256": digest(module), "moduleBytes": len(module),
                             "watSha256": digest(wat.read_bytes()),
                             "locals": {ty: types.count(ty) for ty in ("i32", "i64", "f64", "f32")},
                             "instructions": counts})
        base.write_json(out / "counts.json", {"scope": "main only; parameters excluded; static opcode counts", "rows": rows})
        lines = ["# Independent CLI WAT inspection", "", "Only main (function index 1); parameters excluded from locals. Static counts, not runtime counts.", "Binary hashes equal the corresponding timed modules. Other opcodes and helpers remain in full WAT.", "", "| Case | Level | i32 locals | f64 locals | i64 locals | f64.convert_i32_s | i32 add/sub | f64 add/sub | f64.neg |", "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
        for row in rows:
            ops, local = row["instructions"], row["locals"]
            lines.append(f"| {row['name']} | {row['level']} | {local['i32']} | {local['f64']} | {local['i64']} | {ops['f64.convert_i32_s']} | {ops['i32.add'] + ops['i32.sub']} | {ops['f64.add'] + ops['f64.sub']} | {ops['f64.neg']} |")
        (out / "counts.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
        completed = True
    finally:
        end = matched.hashes()
        base.write_json(out / "hashes-after.json", end)
        base.write_json(out / "stability.json", {"utc_finished": base.utc(), "completed": completed,
            "inputs_match_timed_collection": start == timed_hashes and end == timed_hashes,
            "changed_inputs": matched.changed(start, end)})
    base.require(end == timed_hashes, "Inputs changed during inspection")


if __name__ == "__main__":
    main()
