#[cfg(feature = "binaryen")]
mod binaryen;
#[cfg(feature = "binaryen")]
mod binaryen_guard;
mod cli;

fn main() -> std::process::ExitCode {
    #[cfg(all(feature = "binaryen", not(windows)))]
    if std::env::var_os(binaryen::CORES).is_none() {
        // Binaryen v116 has no thread-count API. Its pool can deadlock on partial
        // thread creation, so default to its single-core path before running any code
        // that can start threads. Explicit settings are checked on active requests.
        // SAFETY: This is the single-threaded binary entry point; neither the Rust
        // startup nor the linked Binaryen library starts threads before this call.
        unsafe { std::env::set_var(binaryen::CORES, binaryen::SINGLE_CORE) };
    }

    cli::main(std::env::args_os().skip(1))
}
