use std::{fs, io, path::Path};
use was_compiler::{
    Compiler, CompilerOptions, ExportOptions, OptimizationLevel, TypedProgram, codegen,
};
use wasmparser::Validator;

pub const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];

pub fn cases() -> [(&'static str, String); 13] {
    fn wide(leaf: &str) -> String {
        (0..4).fold(format!("({leaf})"), |half, _| format!("({half} + {half})"))
    }

    fn expression(result: &str, root: &str) -> String {
        format!("fn main(x: number) -> {result} {{ return {root}; }}\n")
    }

    let closed = "(1.5 + 2.25) * 6.5";
    let open = "(x + 2.25) * 6.5";
    let tiny = format!("0.{}5", "0".repeat(323));
    let huge = format!("1{}", "0".repeat(308));
    [
        ("fraction_closed", expression("number", closed)),
        ("fraction_open", expression("number", open)),
        ("wide_closed", expression("number", &wide(closed))),
        ("wide_open", expression("number", &wide(open))),
        ("bool_closed", expression("bool", "!((3 < 2) == (4 == 4))")),
        ("bool_open", expression("bool", "!((x < 2) == (x != 4))")),
        ("negative_zero", expression("number", "-(1.5 - 1.5)")),
        (
            "subnormal_tie",
            expression("number", &format!("{tiny} * 0.5")),
        ),
        (
            "nonfinite_root",
            expression("number", &format!("{huge} * 10")),
        ),
        ("division_root", expression("number", "1.5 / 2")),
        (
            "integer_local",
            "fn main(x: number) -> bool { let a = 120; let b = a + 23; return b < 200; }\n".into(),
        ),
        (
            "range_direct",
            "fn main(n: number) -> number {
    let total = n - n;
    for i in (n - n)..n { total += i; }
    return total;
}
"
            .into(),
        ),
        (
            "array_growth",
            "fn main(x: number) -> number { let values = [x * 1.5, x]; return values[0]; }\n"
                .into(),
        ),
    ]
}

pub fn compiler(level: OptimizationLevel) -> Compiler {
    let mut compiler = Compiler::with_options(CompilerOptions {
        optimization_level: level,
    });
    compiler.set_exports(ExportOptions::Only(vec!["main".into()]));
    compiler
}

pub fn prepare(source: &str) -> (TypedProgram, [Vec<u8>; 4]) {
    let typed = Compiler::new().analyze(source).unwrap();
    let modules = LEVELS.map(|level| {
        let compiler = compiler(level);
        let bytes = compiler.compile_source(source).unwrap();
        assert_eq!(
            compiler.compile(&typed).unwrap(),
            bytes,
            "{level:?}: {source}"
        );
        Validator::new().validate_all(&bytes).unwrap();
        bytes
    });
    let exports = compiler(OptimizationLevel::None);
    assert_eq!(
        modules[0],
        codegen::compile_with_exports(&typed, exports.exports()).unwrap()
    );
    assert_eq!(modules[1], modules[2]);
    assert_eq!(modules[1], modules[3]);
    (typed, modules)
}

pub fn export<V: std::fmt::Debug, const N: usize>(
    directory: &Path,
    variants: [V; N],
    prepare: impl Fn(&str, &str) -> [Vec<u8>; N],
) -> io::Result<()> {
    use std::io::Write;

    fs::create_dir(directory)?;
    let mut sizes = fs::File::create_new(directory.join("sizes.csv"))?;
    writeln!(sizes, "workload,preset,source_bytes,module_bytes")?;
    for (name, source) in cases() {
        let modules = prepare(name, &source);
        fs::write(directory.join(format!("{name}.was")), &source)?;
        for (level, module) in variants.iter().zip(modules) {
            fs::write(directory.join(format!("{name}-{level:?}.wasm")), &module)?;
            writeln!(sizes, "{name},{level:?},{},{}", source.len(), module.len())?;
        }
    }
    Ok(())
}
