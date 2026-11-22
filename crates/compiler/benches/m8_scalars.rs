#[path = "support/m8_corpus.rs"]
mod corpus;

use std::{
    env,
    fs::File,
    hint::black_box,
    io::{self, BufWriter, Write},
    path::Path,
    time::Instant,
};
use was_compiler::{Compiler, OptimizationLevel, TypedProgram};

const SAMPLES: usize = 7;
const CALLS: usize = 128;
const WARMUP: usize = 64;

#[derive(Clone, Copy)]
enum Boundary<'a> {
    Source(&'a str),
    Typed(&'a TypedProgram),
}

impl Boundary<'_> {
    fn name(self) -> &'static str {
        match self {
            Self::Source(_) => "source",
            Self::Typed(_) => "typed",
        }
    }

    fn compile(self, level: OptimizationLevel) {
        let compiler = corpus::compiler(black_box(level));
        let bytes = match self {
            Self::Source(source) => compiler.compile_source(black_box(source)).unwrap(),
            Self::Typed(typed) => compiler.compile(black_box(typed)).unwrap(),
        };
        drop(black_box(bytes));
    }
}

fn record(directory: &Path, reverse: bool) -> io::Result<()> {
    let mut output = BufWriter::new(File::create_new(directory.join("native.csv"))?);
    let prepared = corpus::cases().map(|(name, source)| {
        let typed = Compiler::new().analyze(&source).unwrap();
        (name, source, typed)
    });
    let mut schedule = Vec::new();
    for (name, source, typed) in &prepared {
        for level in [OptimizationLevel::None, OptimizationLevel::Basic] {
            for boundary in [Boundary::Source(source), Boundary::Typed(typed)] {
                schedule.push((name, level, boundary));
            }
        }
    }
    if reverse {
        schedule.reverse();
    }

    let mut rows = Vec::with_capacity(schedule.len() * SAMPLES);
    for (name, level, boundary) in schedule {
        for _ in 0..WARMUP {
            boundary.compile(level);
        }
        for sample in 1..=SAMPLES {
            let start = Instant::now();
            for _ in 0..CALLS {
                boundary.compile(level);
            }
            let ns = start.elapsed().as_nanos();
            rows.push((name, level, boundary.name(), sample, ns));
        }
    }

    writeln!(output, "workload,preset,boundary,sample,calls,elapsed_ns")?;
    for (name, level, boundary, sample, ns) in rows {
        writeln!(output, "{name},{level:?},{boundary},{sample},{CALLS},{ns}")?;
    }
    output.flush()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.last().is_some_and(|arg| arg == "--bench") {
        args.pop();
    }
    match args.as_slice() {
        [] => {}
        [flag] if flag == "--test" => {
            for (_, source) in corpus::cases() {
                corpus::prepare(&source);
            }
        }
        [flag, directory] if flag == "--prepare" => corpus::export(Path::new(directory))?,
        [flag, directory, order] if flag == "--record-samples" && (order == "forward" || order == "reverse") => {
            record(Path::new(directory), order == "reverse")?;
        }
        _ => return Err("usage: m8_scalars --test | --prepare NEW_DIRECTORY | --record-samples DIRECTORY forward|reverse".into()),
    }
    Ok(())
}
