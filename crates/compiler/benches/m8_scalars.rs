#[path = "support/m8_corpus.rs"]
mod corpus;
#[path = "support/m8_native.rs"]
mod native;

use std::{env, path::Path};
use was_compiler::OptimizationLevel;

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
        [flag, directory] if flag == "--prepare" => {
            corpus::export(Path::new(directory), corpus::LEVELS, |_, source| corpus::prepare(source).1)?;
        }
        [flag, directory, order] if flag == "--record-samples" && (order == "forward" || order == "reverse") => {
            native::record(Path::new(directory), order == "reverse", [OptimizationLevel::None, OptimizationLevel::Basic], |level, boundary| {
                let compiler = corpus::compiler(level);
                match boundary {
                    native::Boundary::Source(source) => compiler.compile_source(source).unwrap(),
                    native::Boundary::Typed(typed) => compiler.compile(typed).unwrap(),
                }
            })?;
        }
        _ => return Err("usage: m8_scalars --test | --prepare NEW_DIRECTORY | --record-samples DIRECTORY forward|reverse".into()),
    }
    Ok(())
}
