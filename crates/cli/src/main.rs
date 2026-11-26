#[cfg(feature = "binaryen")]
mod binaryen;
#[cfg(feature = "binaryen")]
mod binaryen_guard;
mod cli;

fn main() -> std::process::ExitCode {
    cli::main(std::env::args_os().skip(1))
}
