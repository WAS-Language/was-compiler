mod expression_corpus;
use std::{fs, path::PathBuf};
use was_compiler::OptimizationLevel;
use wasmparser::{Operator, Parser, Payload, Validator};

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("artifact directory"));
    fs::create_dir(&out).unwrap();
    let mut records = Vec::new();
    for case in expression_corpus::cases() {
        fs::write(out.join(format!("{}.was", case.name)), &case.source).unwrap();
        let mut enabled = None;
        for level in [
            OptimizationLevel::None,
            OptimizationLevel::Basic,
            OptimizationLevel::Standard,
            OptimizationLevel::Aggressive,
        ] {
            let compiler = expression_corpus::compiler(level);
            let typed = compiler.analyze(&case.source).unwrap();
            let bytes = compiler.compile(&typed).unwrap();
            assert_eq!(bytes, compiler.compile_source(&case.source).unwrap());
            Validator::new().validate_all(&bytes).unwrap();
            if level != OptimizationLevel::None {
                if let Some(reference) = &enabled {
                    assert_eq!(&bytes, reference);
                } else {
                    enabled = Some(bytes.clone());
                }
            }
            let mut locals = 0;
            let mut comparisons = [0; 6];
            let mut negations = 0;
            let mut subtractions = 0;
            for payload in Parser::new(0).parse_all(&bytes) {
                if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                    for local in body.get_locals_reader().unwrap() {
                        locals += local.unwrap().0;
                    }
                    for op in body.get_operators_reader().unwrap() {
                        match op.unwrap() {
                            Operator::I32LtS => comparisons[0] += 1,
                            Operator::I32LeS => comparisons[1] += 1,
                            Operator::I32GtS => comparisons[2] += 1,
                            Operator::I32GeS => comparisons[3] += 1,
                            Operator::I32Eq => comparisons[4] += 1,
                            Operator::I32Ne => comparisons[5] += 1,
                            Operator::F64Neg => negations += 1,
                            Operator::I32Sub => subtractions += 1,
                            _ => {}
                        }
                    }
                }
            }
            assert_eq!(locals, 0, "expression-only, no local storage");
            if level != OptimizationLevel::None {
                if case.name.starts_with("expr_compare") {
                    let count = if case.name.ends_with("_32") { 32 } else { 1 };
                    assert_eq!(comparisons, [count; 6]);
                    assert_eq!(negations, 0);
                } else if case.name.starts_with("expr_negate") {
                    assert_eq!(negations, 0);
                    assert!(subtractions > 0);
                } else {
                    assert_eq!(negations, 1, "zero/min must retain f64.neg");
                }
            }
            let stem = format!("{}-{level:?}", case.name);
            fs::write(out.join(format!("{stem}.wasm")), &bytes).unwrap();
            fs::write(
                out.join(format!("{stem}.wat")),
                wasmprinter::print_bytes(&bytes).unwrap(),
            )
            .unwrap();
            eprintln!("{stem}: locals={locals}, integer_comparisons={comparisons:?}, f64.neg={negations}, i32.sub={subtractions}");
            records.push(format!(
                r#"["{}","{level:?}",{:?},{bytes:?}]"#,
                case.name,
                case.source.as_bytes()
            ));
        }
    }
    fs::write(out.join("payload.json"), format!("[{}]", records.join(","))).unwrap();
    println!("24 valid modules; typed/source parity, enabled byte parity and independent decoded checks passed");
}
