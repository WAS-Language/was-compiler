//! Native compiler source-size workloads, not runtime-parameter timing inputs.
//! Literal statements grow with size; all compilation uses ordinary public APIs.

use std::{
    fmt,
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
};

use was_compiler::{Compiler, TypedProgram};
use wasmparser::Validator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workload {
    ConcatEmpty(usize),
    ConcatSeparated(usize),
    JoinEmpty(usize),
    JoinSeparated(usize),
    ReplaceNoMatch(usize),
    ReplaceExpand(usize),
    ReplaceShrink(usize),
}

pub const CASES: [Workload; 21] = [
    Workload::ConcatEmpty(1),
    Workload::ConcatEmpty(16),
    Workload::ConcatEmpty(64),
    Workload::ConcatSeparated(1),
    Workload::ConcatSeparated(16),
    Workload::ConcatSeparated(64),
    Workload::JoinEmpty(1),
    Workload::JoinEmpty(16),
    Workload::JoinEmpty(64),
    Workload::JoinSeparated(1),
    Workload::JoinSeparated(16),
    Workload::JoinSeparated(64),
    Workload::ReplaceNoMatch(1),
    Workload::ReplaceNoMatch(16),
    Workload::ReplaceNoMatch(64),
    Workload::ReplaceExpand(1),
    Workload::ReplaceExpand(16),
    Workload::ReplaceExpand(64),
    Workload::ReplaceShrink(1),
    Workload::ReplaceShrink(16),
    Workload::ReplaceShrink(64),
];

impl fmt::Display for Workload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConcatEmpty(size) => write!(f, "concat_empty_{size}"),
            Self::ConcatSeparated(size) => write!(f, "concat_separated_{size}"),
            Self::JoinEmpty(size) => write!(f, "join_empty_{size}"),
            Self::JoinSeparated(size) => write!(f, "join_separated_{size}"),
            Self::ReplaceNoMatch(size) => write!(f, "replace_no_match_{size}"),
            Self::ReplaceExpand(size) => write!(f, "replace_expand_{size}"),
            Self::ReplaceShrink(size) => write!(f, "replace_shrink_{size}"),
        }
    }
}

pub fn source(workload: Workload) -> String {
    let mut source = String::from("fn main() -> string {\n");
    match workload {
        Workload::ConcatEmpty(size) | Workload::ConcatSeparated(size) => {
            let separator = match workload {
                Workload::ConcatSeparated(_) => "·",
                _ => "",
            };
            source.push_str("let result = \"\";\n");
            for index in 0..size {
                if index != 0 {
                    source.push_str(&format!(
                        "result = string_concat(result, \"{separator}\");\n"
                    ));
                }
                let fragment = fragment(index);
                source.push_str(&format!(
                    "result = string_concat(result, \"{fragment}\");\n"
                ));
            }
            source.push_str("return result;\n");
        }
        Workload::JoinEmpty(size) | Workload::JoinSeparated(size) => {
            let separator = match workload {
                Workload::JoinSeparated(_) => "·",
                _ => "",
            };
            source.push_str("let parts: string[] = resizable([]);\n");
            for index in 0..size {
                let fragment = fragment(index);
                source.push_str(&format!("parts.push(\"{fragment}\");\n"));
            }
            source.push_str(&format!("return string_join(parts, \"{separator}\");\n"));
        }
        Workload::ReplaceNoMatch(size)
        | Workload::ReplaceExpand(size)
        | Workload::ReplaceShrink(size) => {
            let (needle, replacement) = match workload {
                Workload::ReplaceNoMatch(_) => ("aaaaaac", "x"),
                Workload::ReplaceExpand(_) => ("aa", "éaaa"),
                Workload::ReplaceShrink(_) => ("aa", "b"),
                _ => unreachable!(),
            };
            source.push_str("let result = \"\";\n");
            // Repeat literal calls, rather than a constant-size guest loop. Keeping
            // the source fixed per call isolates native statement-count scaling.
            for _ in 0..size {
                source.push_str(&format!(
                    "result = string_replace_all(\"aaaaaaaaaaaaaaaabaaa\", \
                     \"{needle}\", \"{replacement}\");\n"
                ));
            }
            source.push_str("return result;\n");
        }
    }
    source.push_str("}\n");
    source
}

pub const REPETITIONS: usize = 3;
pub const SAMPLES: usize = 5;
pub const ITERATIONS: usize = 10;
pub const WARMUPS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    Source,
    Typed,
}

impl fmt::Display for Boundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Source => "source_compile",
            Self::Typed => "typed_compile",
        })
    }
}

/// Zero-based repetition selects the order; record indices are one-based.
pub fn sample_pairs(repetition: usize) -> Vec<(Boundary, Workload)> {
    let mut pairs = Vec::with_capacity(CASES.len() * 2);
    for boundary in [Boundary::Source, Boundary::Typed] {
        for workload in CASES {
            pairs.push((boundary, workload));
        }
    }
    if repetition % 2 == 1 {
        pairs.reverse();
    }
    pairs
}

pub struct Sample {
    pub boundary: Boundary,
    pub workload: Workload,
    pub repetition: usize,
    pub sample: usize,
    pub elapsed_ns: u128,
}

impl fmt::Display for Sample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NATIVE_SAMPLE\t{}\t{}\t{}\t{}\t{ITERATIONS}\t{}",
            self.boundary, self.workload, self.repetition, self.sample, self.elapsed_ns
        )
    }
}

#[derive(Debug)]
pub struct PreparedCase {
    pub workload: Workload,
    pub source: String,
    pub typed: TypedProgram,
}

/// The caller supplies an existing fresh directory. Failed exports may leave a
/// partial corpus; create_new prevents overwriting any prior evidence.
pub fn export_corpus(directory: &Path) -> io::Result<Vec<PreparedCase>> {
    if !directory.is_dir() {
        return Err(io::Error::other(
            "corpus output must be an existing directory",
        ));
    }
    let mut prepared = Vec::with_capacity(CASES.len());
    for workload in CASES {
        let source = source(workload);
        let compiler = Compiler::new();
        let typed = compiler.analyze(&source).map_err(io::Error::other)?;
        let bytes = compiler.compile(&typed).map_err(io::Error::other)?;
        Validator::new()
            .validate_all(&bytes)
            .map_err(io::Error::other)?;
        let source_bytes = compiler.compile_source(&source).map_err(io::Error::other)?;
        if source_bytes != bytes {
            return Err(io::Error::other(format!(
                "source/typed byte mismatch: {workload}"
            )));
        }
        for (extension, contents) in [("was", source.as_bytes()), ("wasm", bytes.as_slice())] {
            let path = directory.join(format!("{workload}.{extension}"));
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(contents)?;
            file.sync_all()?;
        }
        prepared.push(PreparedCase {
            workload,
            source,
            typed,
        });
    }
    Ok(prepared)
}

fn fragment(index: usize) -> &'static str {
    match index % 4 {
        0 => "é",
        1 => "",
        2 => "🦀",
        _ => "a",
    }
}
