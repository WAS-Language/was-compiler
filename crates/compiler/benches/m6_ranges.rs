#[path = "support/m6_corpus.rs"]
mod corpus;

use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use divan::{black_box, black_box_drop};
use was_compiler::{OptimizationLevel, TypedProgram};

const ROUNDS: usize = 2;
const SAMPLES: usize = 7;
const CALLS: usize = 128;
const WARMUP_CALLS: usize = 64;

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Source,
    Typed,
}

fn compile_once(boundary: Boundary, source: &str, typed: &TypedProgram, level: OptimizationLevel) {
    let compiler = corpus::compiler(black_box(level));
    let output = match boundary {
        Boundary::Source => compiler.compile_source(black_box(source)).unwrap(),
        Boundary::Typed => compiler.compile(black_box(typed)).unwrap(),
    };
    black_box_drop(output);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.last().is_some_and(|arg| arg == "--bench") {
        args.pop();
    }
    let directory = match args.as_slice() {
        [] => return Ok(()),
        [flag] if flag == "--test" => return Ok(()),
        [flag, directory] if flag == "--record-samples" => PathBuf::from(directory),
        _ => return Err("usage: m6_ranges --record-samples NEW_OUTPUT_DIRECTORY".into()),
    };
    std::fs::create_dir(&directory)?;

    let mut sizes = File::create_new(directory.join("sizes.csv"))?;
    writeln!(sizes, "workload,preset,source_bytes,module_bytes")?;
    let prepared = corpus::CASES
        .map(|(workload, source)| export_case(&directory, &mut sizes, workload, source));
    let prepared = prepared.into_iter().collect::<io::Result<Vec<_>>>()?;

    let mut schedule = Vec::new();
    for (workload, source, typed) in &prepared {
        for boundary in [Boundary::Source, Boundary::Typed] {
            for level in corpus::LEVELS {
                schedule.push((workload, source, typed, boundary, level));
            }
        }
    }

    let mut rows = Vec::with_capacity(schedule.len() * ROUNDS * SAMPLES);
    for round in 1..=ROUNDS {
        if round % 2 == 0 {
            schedule.reverse();
        }
        for &(workload, source, typed, boundary, level) in &schedule {
            for _ in 0..WARMUP_CALLS {
                compile_once(boundary, source, typed, level);
            }
            for sample in 1..=SAMPLES {
                let start = Instant::now();
                for _ in 0..CALLS {
                    compile_once(boundary, source, typed, level);
                }
                let ns = start.elapsed().as_nanos();
                rows.push((workload, boundary, level, round, sample, ns));
            }
        }
    }

    let mut output = BufWriter::new(File::create_new(directory.join("native.csv"))?);
    writeln!(
        output,
        "workload,boundary,preset,round,sample,calls,elapsed_ns"
    )?;
    for (workload, boundary, level, round, sample, ns) in rows {
        writeln!(
            output,
            "{workload},{boundary:?},{level:?},{round},{sample},{CALLS},{ns}"
        )?;
    }
    output.flush()?;
    Ok(())
}

fn export_case<'a>(
    directory: &Path,
    sizes: &mut File,
    workload: &'a str,
    source: &'a str,
) -> io::Result<(&'a str, &'a str, TypedProgram)> {
    let (typed, modules) = corpus::prepare(workload, source);

    File::create_new(directory.join(format!("{workload}.was")))?.write_all(source.as_bytes())?;
    for (level, bytes) in corpus::LEVELS.into_iter().zip(modules) {
        File::create_new(directory.join(format!("{workload}-{level:?}.wasm")))?
            .write_all(&bytes)?;
        writeln!(
            sizes,
            "{workload},{level:?},{},{}",
            source.len(),
            bytes.len()
        )?;
    }
    Ok((workload, source, typed))
}
