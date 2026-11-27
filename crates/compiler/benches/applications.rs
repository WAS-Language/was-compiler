#[path = "../../../tests/support/process.rs"]
mod process;

use std::{
    error::Error,
    fmt::Write as _,
    fs,
    hint::black_box,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

use was_compiler::{Compiler, CompilerOptions, ExportOptions, OptimizationLevel};
use wasmparser::Validator;

const SIZES: [usize; 3] = [16, 128, 1024];
const REPETITIONS: usize = 3;
const SAMPLES: usize = 5;
const WARMUPS: usize = 2;
const ITERATIONS: usize = 10;

const REPORT: &str = r#"fn main() -> string {
    let statuses = dataset();
    let fragments = resizable(["status"]);
    for status in statuses {
        fragments.push(string_concat("item=", status));
    }
    return string_join(fragments, "; ");
}
"#;

const AGGREGATE: &str = r#"fn score(value: number) -> number
requires value >= 0
requires value < 100
{
    if value >= 80 { return value * 3; }
    if value >= 50 { return value * 2; }
    return value;
}

fn main() -> number {
    let readings = dataset();
    let total = 0;
    for index in 0..len(readings) {
        total += score(readings[index]);
    }
    return total;
}
"#;

#[derive(Clone, Copy)]
enum Workload {
    Report,
    Aggregate,
}

impl Workload {
    fn name(self) -> &'static str {
        match self {
            Self::Report => "report",
            Self::Aggregate => "aggregate",
        }
    }

    fn source(self, size: usize) -> String {
        let (body, array_type) = match self {
            Self::Report => (REPORT, "string[]"),
            Self::Aggregate => (AGGREGATE, "number[]"),
        };
        let mut source = format!("{body}\nfn dataset() -> {array_type} {{\n    return [");
        for i in 0..size {
            if i != 0 {
                source.push_str(", ");
            }
            match self {
                Self::Report => {
                    let status = ["ok", "alerta", "ação", "revisão"][i % 4];
                    write!(source, "\"{status}\"").unwrap();
                }
                Self::Aggregate => write!(source, "{}", (i * 37 + 11) % 100).unwrap(),
            }
        }
        source.push_str("];\n}\n");
        source
    }
}

#[derive(Clone, Copy)]
enum Preset {
    O0,
    Default,
}

impl Preset {
    fn name(self) -> &'static str {
        match self {
            Self::O0 => "o0",
            Self::Default => "default",
        }
    }

    fn compiler(self) -> Compiler {
        let mut compiler = match self {
            Self::O0 => Compiler::with_options(CompilerOptions {
                optimization_level: OptimizationLevel::None,
            }),
            Self::Default => Compiler::new(),
        };
        compiler.set_exports(ExportOptions::Only(vec!["main".into()]));
        compiler
    }
}

struct Case {
    name: String,
    source: String,
    preset: Preset,
    wasm_bytes: usize,
}

fn prepare(directory: &Path) -> Result<Vec<Case>, Box<dyn Error>> {
    let mut cases = Vec::new();
    for workload in [Workload::Report, Workload::Aggregate] {
        for size in SIZES {
            let source = workload.source(size);
            let name = format!("{}-{size}", workload.name());
            fs::write(directory.join(format!("{name}.was")), &source)?;
            for preset in [Preset::O0, Preset::Default] {
                let name = format!("{name}-{}", preset.name());
                let bytes = preset.compiler().compile_source(&source)?;
                Validator::new().validate_all(&bytes)?;
                fs::write(directory.join(format!("{name}.wasm")), &bytes)?;
                cases.push(Case {
                    name,
                    source: source.clone(),
                    preset,
                    wasm_bytes: bytes.len(),
                });
            }
        }
    }
    Ok(cases)
}

fn compile_once(case: &Case) {
    let bytes = case
        .preset
        .compiler()
        .compile_source(black_box(&case.source))
        .expect("the prepared source already compiled");
    drop(black_box(bytes));
}

fn native_samples(cases: &[Case]) -> String {
    let mut records = Vec::new();
    for repetition in 0..REPETITIONS {
        for index in 0..cases.len() {
            let index = if repetition % 2 == 0 {
                index
            } else {
                cases.len() - 1 - index
            };
            let case = &cases[index];
            for _ in 0..WARMUPS {
                compile_once(case);
            }
            for sample in 0..SAMPLES {
                let start = Instant::now();
                for _ in 0..ITERATIONS {
                    compile_once(case);
                }
                records.push((case, repetition + 1, sample + 1, start.elapsed().as_nanos()));
            }
        }
    }

    let mut csv =
        String::from("case,source_bytes,wasm_bytes,repetition,sample,iterations,elapsed_ns\n");
    for (case, repetition, sample, elapsed_ns) in records {
        writeln!(
            csv,
            "{},{},{},{repetition},{sample},{ITERATIONS},{elapsed_ns}",
            case.name,
            case.source.len(),
            case.wasm_bytes
        )
        .unwrap();
    }
    csv
}

fn node(root: &Path, directory: &Path, mode: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let output = process::output(
        Command::new("node")
            .current_dir(root)
            .arg("tests/application-bench.mjs")
            .arg(mode)
            .arg(directory),
        None,
        Duration::from_secs(120),
    );
    if !output.status.success() {
        return Err(format!("Node {mode}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(output.stdout)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.last().is_some_and(|arg| arg == "--bench") {
        args.pop();
    }
    let usage = "usage: cargo bench --offline -p was-compiler --bench applications -- --check|--record FRESH_DIRECTORY";
    let [mode, directory] = args.as_slice() else {
        if args.is_empty() {
            println!("{usage}");
            return Ok(());
        }
        return Err(usage.into());
    };
    if mode != "--check" && mode != "--record" {
        return Err(usage.into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join(directory);
    fs::create_dir(&directory)?;
    let cases = prepare(&directory)?;
    node(&root, &directory, "check")?;
    if mode == "--record" {
        let native = native_samples(&cases);
        let runtime = node(&root, &directory, "timing")?;
        fs::write(directory.join("native.csv"), native)?;
        fs::write(directory.join("runtime.json"), runtime)?;
    }
    println!(
        "validated {} application cases in {}",
        cases.len(),
        directory.display()
    );
    Ok(())
}
