use was_compiler::{
    Compiler, CompilerOptions, ExportOptions, OptimizationLevel, TypedProgram, codegen,
};
use wasmparser::Validator;

pub const CASES: [(&str, &str); 3] = [
    (
        "direct",
        "fn sum(n: number) -> number {
    let total = n - n;
    for i in (n - n)..n { total += i; }
    return total;
}
",
    ),
    (
        "binding",
        "fn sum(n: number) -> number {
    let total = n - n;
    let values = (n - n)..n;
    for i in values { total += i; }
    return total;
}
",
    ),
    (
        "call",
        "fn make_range(n: number) -> range { return (n - n)..n; }
fn sum(n: number) -> number {
    let total = n - n;
    for i in make_range(n) { total += i; }
    return total;
}
",
    ),
];

pub const LEVELS: [OptimizationLevel; 4] = [
    OptimizationLevel::None,
    OptimizationLevel::Basic,
    OptimizationLevel::Standard,
    OptimizationLevel::Aggressive,
];

pub fn compiler(level: OptimizationLevel) -> Compiler {
    let mut compiler = Compiler::with_options(CompilerOptions {
        optimization_level: level,
    });
    compiler.set_exports(ExportOptions::Only(vec!["sum".into()]));
    compiler
}

pub fn prepare(workload: &str, source: &str) -> (TypedProgram, [Vec<u8>; 4]) {
    let typed = Compiler::new().analyze(source).unwrap();
    let modules = LEVELS.map(|level| {
        let compiler = compiler(level);
        let bytes = compiler.compile_source(source).unwrap();
        assert_eq!(
            compiler.compile(&typed).unwrap(),
            bytes,
            "{workload}/{level:?}"
        );
        Validator::new().validate_all(&bytes).unwrap();
        bytes
    });
    let baseline = compiler(OptimizationLevel::None);
    assert_eq!(
        modules[0],
        codegen::compile_with_exports(&typed, baseline.exports()).unwrap(),
        "{workload}: baseline"
    );
    assert_eq!(modules[0] == modules[1], workload != "direct", "{workload}");
    assert_eq!(modules[1], modules[2], "{workload}");
    assert_eq!(modules[1], modules[3], "{workload}");
    (typed, modules)
}
