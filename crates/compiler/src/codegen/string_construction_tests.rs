//! Deterministic #79 construction evidence through the private paired-module seam.
use crate::{Compiler, CompilerOptions, OptimizationLevel};
use std::{path::PathBuf, process::Command, time::Duration};
use wasmparser::{Parser, Payload, Validator};

const SOURCE: &str = include_str!("../../../../tests/support/m5_construction_corpus.was");

fn baseline_compiler() -> Compiler {
    Compiler::with_options(CompilerOptions {
        optimization_level: OptimizationLevel::None,
    })
}

#[test]
fn construction_corpus_is_valid_deterministic_and_observations_are_private() {
    let compiler = baseline_compiler();
    let [ordinary, observed] =
        super::resizable::construction_observations::modules(&compiler, SOURCE);
    assert_eq!(ordinary, compiler.compile_source(SOURCE).unwrap());
    assert_eq!(
        [ordinary.clone(), observed.clone()],
        super::resizable::construction_observations::modules(&compiler, SOURCE)
    );
    for (bytes, private) in [(&ordinary, false), (&observed, true)] {
        Validator::new().validate_all(bytes).unwrap();
        let mut observation_exports = 0;
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::ExportSection(exports) = payload.unwrap() {
                for export in exports {
                    let export = export.unwrap();
                    if export.name.starts_with("__m5_") {
                        assert_eq!(export.kind, wasmparser::ExternalKind::Global);
                        observation_exports += 1;
                    }
                }
            }
        }
        assert_eq!(observation_exports, if private { 15 } else { 0 });
    }
}

#[test]
#[ignore = "requires Node.js; writes deterministic #79 accounting evidence"]
fn paired_fragment_construction_is_linear() {
    let [ordinary, observed] =
        super::resizable::construction_observations::modules(&baseline_compiler(), SOURCE);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("target/m5-construction");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, bytes) in [("ordinary", ordinary), ("observed", observed)] {
        std::fs::write(directory.join(format!("{name}.wasm")), bytes).unwrap();
    }
    let output = crate::test_process::output(
        Command::new("node")
            .arg(root.join("tests/support/m5_construction.cjs"))
            .arg(&directory),
        None,
        Duration::from_secs(120),
    );
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(directory.join("accounting.json"), &output.stdout).unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
