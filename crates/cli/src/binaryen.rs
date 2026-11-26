use std::{env, fs, io};

use thiserror::Error;
use was_compiler::OptimizationLevel;
use wasm_opt::{FileType, OptimizationError, OptimizationOptions, Pass, ShrinkLevel};

use crate::binaryen_guard;

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error("Binaryen optimization error: {0}")]
    Scratch(#[from] io::Error),
    #[error("Binaryen optimization error: temporary directory path is not UTF-8")]
    NonUtf8Scratch,
    #[error("Binaryen optimization error: unset {0} or {1}")]
    Environment(&'static str, &'static str),
    #[error("Binaryen optimization error: {0}")]
    Optimize(#[from] OptimizationError),
    #[error("Binaryen output check failed: {0}")]
    Output(binaryen_guard::GuardError),
}

pub(super) fn optimize(wasm: &[u8], level: OptimizationLevel) -> Result<Vec<u8>, Error> {
    check_environment()?;

    let mut options = match level {
        OptimizationLevel::None => unreachable!("O0 bypasses Binaryen"),
        OptimizationLevel::Basic => OptimizationOptions::new_opt_level_1(),
        OptimizationLevel::Standard => OptimizationOptions::new_opt_level_2(),
        OptimizationLevel::Aggressive => OptimizationOptions::new_opt_level_3(),
    };

    // local-peephole-v1: the stock pipelines remove unused registered imports.
    options
        .mvp_features_only()
        .shrink_level(ShrinkLevel::Level0)
        .fast_math(false)
        .debug_info(false)
        .add_default_passes(false)
        .add_pass(Pass::OptimizeInstructions)
        .reader_file_type(FileType::Wasm)
        .writer_file_type(FileType::Wasm);

    let scratch = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tempfile::Builder::new()
                .permissions(fs::Permissions::from_mode(0o700))
                .tempdir()
        }
        #[cfg(not(unix))]
        {
            tempfile::tempdir()
        }
    }?;

    // The pinned binding panics when converting a non-UTF-8 path to C++.
    if scratch.path().to_str().is_none() {
        return Err(Error::NonUtf8Scratch);
    }
    let input = scratch.path().join("input.wasm");
    let output = scratch.path().join("output.wasm");

    fs::write(&input, wasm)?;
    options.run(&input, &output)?;
    let optimized = fs::read(&output)?;

    binaryen_guard::check(wasm, &optimized).map_err(Error::Output)?;

    Ok(optimized)
}

fn check_environment() -> Result<(), Error> {
    // v116 can dump unguarded modules into the working directory in pass-debug mode.
    const PASS_DEBUG: &str = "BINARYEN_PASS_DEBUG";
    if env::var_os(PASS_DEBUG).is_some_and(|value| value != "0") {
        return Err(Error::Environment(PASS_DEBUG, "set it to 0"));
    }

    // The native thread pool parses this with std::stoi outside the binding's error handling.
    const CORES: &str = "BINARYEN_CORES";
    if let Some(value) = env::var_os(CORES)
        && !value
            .to_str()
            .and_then(|value| value.parse::<i32>().ok())
            .is_some_and(|count| count > 0)
    {
        return Err(Error::Environment(CORES, "set it to a positive integer"));
    }

    Ok(())
}
