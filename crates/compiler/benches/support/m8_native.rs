use std::{
    fmt::Debug,
    fs::File,
    hint::black_box,
    io::{self, BufWriter, Write},
    path::Path,
    time::Instant,
};
use was_compiler::{Compiler, TypedProgram};

use super::corpus;

const SAMPLES: usize = 7;
const CALLS: usize = 128;
const WARMUP: usize = 64;

#[derive(Clone, Copy)]
pub enum Boundary<'a> {
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
}

pub fn record<V: Copy + Debug>(
    directory: &Path,
    reverse: bool,
    variants: [V; 2],
    compile: impl Fn(V, Boundary<'_>) -> Vec<u8>,
) -> io::Result<()> {
    let mut output = BufWriter::new(File::create_new(directory.join("native.csv"))?);
    let prepared = corpus::cases().map(|(name, source)| {
        let typed = Compiler::new().analyze(&source).unwrap();
        (name, source, typed)
    });
    let mut schedule = Vec::new();
    for (name, source, typed) in &prepared {
        for variant in variants {
            for boundary in [Boundary::Source(source), Boundary::Typed(typed)] {
                schedule.push((name, variant, boundary));
            }
        }
    }
    if reverse {
        schedule.reverse();
    }

    let invoke =
        |variant, boundary| drop(black_box(compile(black_box(variant), black_box(boundary))));
    let mut rows = Vec::with_capacity(schedule.len() * SAMPLES);
    for (name, variant, boundary) in schedule {
        for _ in 0..WARMUP {
            invoke(variant, boundary);
        }
        for sample in 1..=SAMPLES {
            let start = Instant::now();
            for _ in 0..CALLS {
                invoke(variant, boundary);
            }
            let ns = start.elapsed().as_nanos();
            rows.push((name, variant, boundary.name(), sample, ns));
        }
    }

    writeln!(output, "workload,preset,boundary,sample,calls,elapsed_ns")?;
    for (name, variant, boundary, sample, ns) in rows {
        writeln!(
            output,
            "{name},{variant:?},{boundary},{sample},{CALLS},{ns}"
        )?;
    }
    output.flush()
}
