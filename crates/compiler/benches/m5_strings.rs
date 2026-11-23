#[path = "support/m5_corpus.rs"]
mod corpus;

use std::{
    io::{self, Write},
    path::Path,
    time::Instant,
};

use corpus::{Boundary, CASES, PreparedCase, Workload};
use divan::{Bencher, black_box, black_box_drop};
use was_compiler::{Compiler, CompilerOptions, OptimizationLevel};
use wasmparser::Validator;

fn baseline_compiler() -> Compiler {
    Compiler::with_options(CompilerOptions {
        optimization_level: OptimizationLevel::None,
    })
}

fn validated_source(workload: Workload) -> String {
    let source = corpus::source(workload);
    let bytes = baseline_compiler().compile_source(&source).unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    source
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    // Cargo can append its harness flag even for harness=false benchmarks.
    if args.last().is_some_and(|arg| arg == "--bench") {
        args.pop();
    }
    if args.iter().any(|arg| arg == "--record-samples") {
        match args.as_slice() {
            [flag, directory] if flag == "--record-samples" => {
                record_samples(Path::new(directory))?;
                return Ok(());
            }
            _ => {
                return Err(
                    "usage: --record-samples EXISTING_FRESH_DIRECTORY (no other options)".into(),
                );
            }
        }
    }
    eprintln!("numeric_preset=None; native source-size corpus; workload,source_bytes,module_bytes");
    for workload in CASES {
        let source = validated_source(workload);
        let bytes = baseline_compiler().compile_source(&source).unwrap();
        eprintln!("{workload},{},{}", source.len(), bytes.len());
    }
    divan::main();
    Ok(())
}

fn compile_once(boundary: Boundary, case: &PreparedCase) {
    match boundary {
        Boundary::Source => {
            black_box_drop(
                baseline_compiler()
                    .compile_source(black_box(&case.source))
                    .unwrap(),
            );
        }
        Boundary::Typed => {
            black_box_drop(baseline_compiler().compile(black_box(&case.typed)).unwrap());
        }
    }
}

fn record_samples(directory: &Path) -> io::Result<()> {
    // Preparation, parity checks, Validator and create_new exports all finish
    // before any warmup or timer. No observation-enabled compilation is used.
    let prepared = corpus::export_corpus(directory)?;
    let mut records = Vec::with_capacity(CASES.len() * 2 * corpus::REPETITIONS * corpus::SAMPLES);
    eprintln!(
        "numeric_preset=None; repetitions=3; samples=5; iterations=10; warmups=2; timer=Instant; odd_repetitions=forward; even_repetitions=reverse"
    );
    for repetition in 0..corpus::REPETITIONS {
        for (boundary, workload) in corpus::sample_pairs(repetition) {
            let case = prepared
                .iter()
                .find(|case| case.workload == workload)
                .unwrap();
            for _ in 0..corpus::WARMUPS {
                compile_once(boundary, case);
            }
            for sample in 0..corpus::SAMPLES {
                let start = Instant::now();
                for _ in 0..corpus::ITERATIONS {
                    compile_once(boundary, case);
                }
                let elapsed_ns = start.elapsed().as_nanos();
                records.push(corpus::Sample {
                    boundary,
                    workload,
                    repetition: repetition + 1,
                    sample: sample + 1,
                    elapsed_ns,
                });
            }
        }
    }
    // Defer formatting and stdout writes until all timed regions are complete.
    let mut output = io::BufWriter::new(io::stdout().lock());
    for record in records {
        writeln!(output, "{record}")?;
    }
    output.flush()
}

#[divan::bench(args = CASES)]
fn source_compile(bencher: Bencher, workload: Workload) {
    let source = validated_source(workload);
    bencher.bench_local(|| {
        // Returning () keeps Wasm-buffer destruction inside the timed closure.
        black_box_drop(
            baseline_compiler()
                .compile_source(black_box(&source))
                .unwrap(),
        );
    });
}

#[divan::bench(args = CASES)]
fn typed_compile(bencher: Bencher, workload: Workload) {
    let source = corpus::source(workload);
    let typed = baseline_compiler().analyze(&source).unwrap();
    let bytes = baseline_compiler().compile(&typed).unwrap();
    Validator::new().validate_all(&bytes).unwrap();
    bencher.bench_local(|| {
        black_box_drop(baseline_compiler().compile(black_box(&typed)).unwrap());
    });
}
