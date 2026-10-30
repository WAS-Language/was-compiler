#[path = "../benches/support/m5_corpus.rs"]
mod corpus;

#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{process::Command, time::Duration};

use was_compiler::{Compiler, CompilerOptions, OptimizationLevel};
use wasmparser::Validator;

#[test]
fn source_size_families_are_deterministic_valid_and_grow() {
    assert_eq!(corpus::CASES.len(), 21);
    for family in corpus::CASES.as_chunks::<3>().0 {
        let mut previous = (0, 0);
        for &workload in family {
            let source = corpus::source(workload);
            assert_eq!(source, corpus::source(workload), "{workload}");
            let compiler = Compiler::new();
            let typed = compiler.analyze(&source).unwrap();
            let bytes = compiler.compile(&typed).unwrap();
            Validator::new().validate_all(&bytes).unwrap();
            assert_eq!(
                bytes,
                compiler.compile_source(&source).unwrap(),
                "{workload}"
            );
            assert_eq!(bytes, compiler.compile(&typed).unwrap(), "{workload}");
            assert!(source.len() > previous.0, "{workload}");
            assert!(bytes.len() > previous.1, "{workload}");
            previous = (source.len(), bytes.len());
        }
    }
}

#[test]
fn raw_sample_schedule_and_records_are_complete_and_alternate_order() {
    let forward = corpus::sample_pairs(0);
    assert_eq!(forward.len(), 42);
    assert_eq!(forward[0].0.to_string(), "source_compile");
    assert_eq!(forward[0].1.to_string(), "concat_empty_1");
    let reversed = forward.iter().rev().copied().collect::<Vec<_>>();
    assert_eq!(corpus::sample_pairs(1), reversed);
    assert_eq!(corpus::sample_pairs(2), forward);
    assert_eq!(corpus::REPETITIONS, 3);
    assert_eq!(corpus::SAMPLES, 5);
    assert_eq!(corpus::ITERATIONS, 10);
    assert_eq!(corpus::WARMUPS, 2);
    assert_eq!(forward.len() * corpus::REPETITIONS * corpus::SAMPLES, 630);
    let line = corpus::Sample {
        boundary: forward[0].0,
        workload: forward[0].1,
        repetition: 1,
        sample: 1,
        elapsed_ns: 123456,
    }
    .to_string();
    assert_eq!(
        line,
        "NATIVE_SAMPLE\tsource_compile\tconcat_empty_1\t1\t1\t10\t123456"
    );
}

#[test]
fn exported_corpus_matches_ordinary_compilation_and_never_overwrites() {
    let directory =
        std::env::temp_dir().join(format!("was-m5-native-export-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    {
        let prepared = corpus::export_corpus(&directory).unwrap();
        assert_eq!(prepared.len(), 21);
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 42);
        for case in prepared {
            let name = case.workload.to_string();
            assert_eq!(
                std::fs::read_to_string(directory.join(format!("{name}.was"))).unwrap(),
                case.source
            );
            let bytes = std::fs::read(directory.join(format!("{name}.wasm"))).unwrap();
            Validator::new().validate_all(&bytes).unwrap();
            assert_eq!(bytes, Compiler::new().compile(&case.typed).unwrap());
            assert_eq!(bytes, Compiler::new().compile_source(&case.source).unwrap());
        }
        let sentinel = directory.join("concat_empty_1.was");
        std::fs::write(&sentinel, "do not overwrite").unwrap();
        assert_eq!(
            corpus::export_corpus(&directory).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            std::fs::read_to_string(sentinel).unwrap(),
            "do not overwrite"
        );
    }
}

const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];

#[test]
fn analysis_is_preset_invariant_and_enabled_presets_have_identical_bytes() {
    for workload in corpus::CASES {
        let source = corpus::source(workload);
        let baseline = Compiler::new().analyze(&source).unwrap();
        let mut enabled_bytes = None;
        for optimization_level in LEVELS {
            let compiler = Compiler::with_options(CompilerOptions { optimization_level });
            let typed = compiler.analyze(&source).unwrap();
            assert_eq!(typed, baseline, "{workload}: {optimization_level:?}");
            let bytes = compiler.compile(&typed).unwrap();
            Validator::new().validate_all(&bytes).unwrap();
            assert_eq!(
                bytes,
                compiler.compile_source(&source).unwrap(),
                "{workload}"
            );
            if optimization_level != OptimizationLevel::None {
                if let Some(expected) = &enabled_bytes {
                    assert_eq!(&bytes, expected, "{workload}: {optimization_level:?}");
                } else {
                    enabled_bytes = Some(bytes);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires Node.js on PATH; correctness only, not runtime timing"]
fn ordinary_modules_execute_against_independent_string_results_at_all_presets() {
    let mut cases = Vec::new();
    for workload in corpus::CASES {
        for optimization_level in LEVELS {
            let bytes = Compiler::with_options(CompilerOptions { optimization_level })
                .compile_source(&corpus::source(workload))
                .unwrap();
            cases.push(format!(
                "[\"{workload}\", \"{optimization_level:?}\", {bytes:?}]"
            ));
        }
    }
    let input = format!("[{}]", cases.join(","));
    let script = r#"
const assert = require('node:assert/strict');
const cases = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const decoder = new TextDecoder('utf-8', { fatal: true });
assert.equal(cases.length, 84);
for (const [name, level, bytes] of cases) {
    const size = Number(name.slice(name.lastIndexOf('_') + 1));
    let expected;
    if (name.startsWith('concat_') || name.startsWith('join_')) {
        const fragments = Array.from({ length: size }, (_, i) => ['é', '', '🦀', 'a'][i % 4]);
        expected = fragments.join(name.includes('_separated_') ? '·' : '');
    } else if (name.startsWith('replace_no_match_')) {
        expected = 'aaaaaaaaaaaaaaaabaaa';
    } else if (name.startsWith('replace_expand_')) {
        // Nine nonoverlapping aa matches; replacement text is never rescanned.
        expected = 'éaaa'.repeat(8) + 'b' + 'éaaa' + 'a';
    } else {
        assert.ok(name.startsWith('replace_shrink_'), name);
        expected = 'bbbbbbbb' + 'b' + 'ba';
    }
    const instance = new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array(bytes)));
    const handle = instance.exports.main();
    const memory = instance.exports.memory.buffer;
    const view = new DataView(memory);
    const pointer = view.getUint32(handle, true);
    const length = view.getUint32(handle + 4, true);
    const actual = decoder.decode(new Uint8Array(memory, pointer, length));
    assert.equal(actual, expected, `${name}/${level}`);
    assert.equal(length, Buffer.byteLength(expected), `${name}/${level} byte length`);
}
console.log('84 ordinary M5 native-corpus results passed');
"#;
    let output = test_process::output(
        Command::new("node").args(["-e", script]),
        Some(input.as_bytes()),
        Duration::from_secs(30),
    );
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("84 ordinary M5 native-corpus results passed")
    );
}
