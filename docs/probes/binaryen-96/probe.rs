use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use was_compiler::{Compiler, ExportOptions, OptimizationLevel, Type};
use wasm_opt::{OptimizationOptions, Pass};
use wasmparser::types::{EntityType, TypesRef};
use wasmparser::{
    FuncType, GlobalType, MemoryType, Parser, Payload, TableType, Validator, WasmFeatures,
};

#[path = "../../../tests/support/process.rs"]
mod process;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    Stock,
    LocalPeepholeV1,
}

enum Case {
    UnusedImport,
    MemoryOnly,
    OrderedImports,
}

impl Case {
    fn name(&self) -> &'static str {
        match self {
            Self::UnusedImport => "unused-import",
            Self::MemoryOnly => "memory-only",
            Self::OrderedImports => "ordered-imports",
        }
    }

    fn compile(&self, preset: OptimizationLevel) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut compiler = Compiler::new();
        compiler.set_optimization_level(preset);

        let source = match self {
            Self::UnusedImport => {
                compiler.register_host_function(
                    "unused",
                    "host",
                    "unused",
                    vec![Type::Number],
                    Type::Bool,
                )?;
                "fn main() -> number { return 1; }"
            }
            Self::OrderedImports => {
                compiler.register_host_function(
                    "unused",
                    "z",
                    "unused",
                    vec![Type::Bool],
                    Type::Void,
                )?;
                compiler.register_host_function(
                    "used",
                    "a",
                    "used",
                    vec![Type::Number],
                    Type::Number,
                )?;
                "fn main() -> number { return used(42); }"
            }
            Self::MemoryOnly => {
                compiler.set_exports(ExportOptions::None);
                "fn main() -> number { let values = [10, 20]; return values[0]; }"
            }
        };

        Ok(compiler.compile_source(source)?)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ExternalType {
    Func(FuncType),
    ExactFunc(FuncType),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
    Tag(FuncType),
}

impl ExternalType {
    fn resolve(types: TypesRef<'_>, entity: EntityType) -> Self {
        match entity {
            EntityType::Func(id) => Self::Func(types[id].unwrap_func().clone()),
            EntityType::FuncExact(id) => Self::ExactFunc(types[id].unwrap_func().clone()),
            EntityType::Table(ty) => Self::Table(ty),
            EntityType::Memory(ty) => Self::Memory(ty),
            EntityType::Global(ty) => Self::Global(ty),
            EntityType::Tag(id) => Self::Tag(types[id].unwrap_func().clone()),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Interface<'a> {
    imports: Vec<(&'a str, &'a str, ExternalType)>,
    exports: Vec<(&'a str, ExternalType)>,
}

fn inspect(bytes: &[u8]) -> Result<Interface<'_>, Box<dyn Error>> {
    let types = Validator::new_with_features(WasmFeatures::MVP).validate_all(bytes)?;
    let types = types.as_ref();
    let mut interface = Interface::default();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import?;
                    let entity = types
                        .entity_type_from_import(&import)
                        .ok_or("unresolved import")?;
                    interface.imports.push((
                        import.module,
                        import.name,
                        ExternalType::resolve(types, entity),
                    ));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export?;
                    let entity = types
                        .entity_type_from_export(&export)
                        .ok_or("unresolved export")?;
                    interface
                        .exports
                        .push((export.name, ExternalType::resolve(types, entity)));
                }
            }
            Payload::StartSection { .. } => return Err("unexpected start function".into()),
            _ => {}
        }
    }

    Ok(interface)
}

fn check_output(bytes: &[u8], expected: &Interface<'_>) -> Result<(), Box<dyn Error>> {
    let actual = inspect(bytes)?;

    if actual != *expected {
        return Err(
            format!("external interface changed: expected {expected:?}; got {actual:?}").into(),
        );
    }

    Ok(())
}

fn run_js(profile: Profile, level: u8, input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("optimize.mjs");
    let mut command = Command::new("node");
    command
        .arg(script)
        .arg(level.to_string())
        .arg(input)
        .arg(output);
    if profile == Profile::LocalPeepholeV1 {
        command.arg("local-peephole-v1");
    }
    let result = process::output(&mut command, None, Duration::from_secs(15));

    if !result.status.success() {
        return Err(format!(
            "binaryen.js exited with {}: {}",
            result.status,
            String::from_utf8_lossy(&result.stderr),
        )
        .into());
    }

    Ok(())
}

fn report_parity(rust: &[u8], js: &[u8]) {
    if rust == js {
        println!("  optimized bytes identical ({} bytes)", rust.len());
    } else {
        let offset = rust
            .iter()
            .zip(js)
            .position(|(a, b)| a != b)
            .unwrap_or(rust.len().min(js.len()));
        println!(
            "  optimized bytes differ at offset {offset} (Rust {}, JS {})",
            rust.len(),
            js.len()
        );
    }
}

fn run_case(
    case: Case,
    profile: Profile,
    level: u8,
    preset: OptimizationLevel,
    native: &OptimizationOptions,
    output_dir: &Path,
) -> Result<usize, Box<dyn Error>> {
    let name = case.name();
    let input = output_dir.join(format!("{name}-O{level}-input.wasm"));
    let rust_output = output_dir.join(format!("{name}-O{level}-rust.wasm"));
    let js_output = output_dir.join(format!("{name}-O{level}-js.wasm"));
    let bytes = case.compile(preset)?;
    let expected = inspect(&bytes)?;
    fs::write(&input, &bytes)?;
    println!("{name} O{level}: input {} bytes; {expected:?}", bytes.len());

    native.run(&input, &rust_output)?;
    run_js(profile, level, &input, &js_output)?;
    let rust = fs::read(rust_output)?;
    let js = fs::read(js_output)?;
    let mut rejected = 0;

    for (binding, optimized) in [("Rust", &rust), ("JS", &js)] {
        match check_output(optimized, &expected) {
            Ok(()) => println!("  {binding}: PASS ({} bytes)", optimized.len()),
            Err(error) => {
                rejected += 1;
                println!("  {binding}: FAIL: {error}");
            }
        }
    }
    report_parity(&rust, &js);

    if profile == Profile::LocalPeepholeV1 && rust != js {
        rejected += 1;
    }
    Ok(rejected)
}

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let output_dir = PathBuf::from(
        args.next()
            .ok_or("usage: binaryen-96-probe NEW_OUTPUT_DIRECTORY")?,
    );
    let profile = match args.next() {
        None => Profile::Stock,
        Some(name) if name == "local-peephole-v1" && args.next().is_none() => {
            Profile::LocalPeepholeV1
        }
        _ => {
            return Err("usage: binaryen-96-probe NEW_OUTPUT_DIRECTORY [local-peephole-v1]".into());
        }
    };

    fs::create_dir(&output_dir)?;
    let mut rejected = 0;

    for (level, preset, mut native) in [
        (
            1,
            OptimizationLevel::Basic,
            OptimizationOptions::new_opt_level_1(),
        ),
        (
            2,
            OptimizationLevel::Standard,
            OptimizationOptions::new_opt_level_2(),
        ),
        (
            3,
            OptimizationLevel::Aggressive,
            OptimizationOptions::new_opt_level_3(),
        ),
    ] {
        native.mvp_features_only();

        let mut cases = vec![Case::UnusedImport, Case::MemoryOnly];
        if profile == Profile::LocalPeepholeV1 {
            native
                .add_default_passes(false)
                .add_pass(Pass::OptimizeInstructions);
            cases.push(Case::OrderedImports);
        }
        for case in cases {
            rejected += run_case(case, profile, level, preset, &native, &output_dir)?;
        }
    }

    if rejected != 0 {
        eprintln!("{rejected} binding outputs failed the feasibility contract");
        return Ok(ExitCode::from(2));
    }

    Ok(ExitCode::SUCCESS)
}
