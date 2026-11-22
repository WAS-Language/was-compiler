#[path = "../../../tests/support/m6_adapter_cases.rs"]
mod cases;
#[path = "../../../tests/support/process.rs"]
mod test_process;

use cases::{CASES, EXPORTS, PRIVATE_HOST_HINT, SOURCE, Selection};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use was_compiler::{Type, diagnostic};

const FLAGS: [&str; 4] = ["-O0", "-O1", "-O2", "-O3"];
const HOSTS: [&str; 4] = [
    "--host",
    "checkpoint=range.for(number)->void",
    "--host",
    "unused=by.in()->void",
];

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("was-m6-cli-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn cli(&self, level: Option<&str>, args: &[&str]) -> Output {
        test_process::output(
            Command::new(env!("CARGO_BIN_EXE_was-compiler"))
                .current_dir(&self.0)
                .arg("input.was")
                .args(level)
                .args(HOSTS)
                .args(args),
            None,
            Duration::from_secs(15),
        )
    }

    fn compile(&self, level: Option<&str>, selection: Selection, output: &str) -> Output {
        let mut args = vec!["-o", output];

        match selection {
            Selection::Selected => {
                for name in EXPORTS {
                    args.extend(["--export", name]);
                }
            }
            Selection::None => args.push("--no-exports"),
            Selection::All => {}
        }

        let result = self.cli(level, &args);
        assert!(
            result.status.success(),
            "{level:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        result
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_bytes_and_wat_match_core_in_each_preset() {
    let scratch = Scratch::new();
    let presets = FLAGS.map(Some).into_iter().zip(0..4);
    for ((case, source, selection), native) in CASES.into_iter().zip(cases::core_matrix()) {
        fs::write(scratch.0.join("input.was"), source).unwrap();

        for (level, index) in presets.clone().chain([(None, 0)]) {
            let output = scratch.compile(level, selection, "output.wasm");
            assert!(output.stdout.is_empty());
            assert_eq!(
                fs::read(scratch.0.join("output.wasm")).unwrap(),
                native[index],
                "{case}: {level:?}"
            );
            let wat = scratch.compile(level, selection, "-");
            assert_eq!(
                wat.stdout,
                wasmprinter::print_bytes(&native[index]).unwrap().as_bytes()
            );
        }
    }
}

#[test]
fn cli_range_diagnostics_match_core_in_each_preset() {
    let scratch = Scratch::new();
    for (source, baseline) in cases::diagnostics() {
        fs::write(scratch.0.join("input.was"), source).unwrap();
        let mut expected = diagnostic::render("input.was", source, &baseline);
        if source == SOURCE {
            expected.push_str("\nSelect only compatible functions with --export NAME, or use --no-exports to keep all user functions internal.");
        }
        for flag in FLAGS {
            let output = scratch.cli(Some(flag), &[]);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, format!("{expected}\n").as_bytes());
        }
    }

    for (params, result, declaration) in [
        (vec![Type::Range], Type::Void, "bad=range.for(range)->void"),
        (vec![], Type::Range, "bad=range.for()->range"),
    ] {
        let error = cases::compiler()
            .register_host_function("bad", "range", "for", params, result)
            .unwrap_err();
        let expected =
            format!("host registration error in {declaration:?}: {error}{PRIVATE_HOST_HINT}\n");
        for flag in FLAGS {
            let output = scratch.cli(Some(flag), &["--host", declaration]);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, expected.as_bytes());
        }
    }
}

#[test]
#[ignore = "requires Node.js"]
fn cli_range_modules_execute_with_expected_values_effects_and_traps() {
    let scratch = Scratch::new();
    for (case, source, selection) in CASES {
        fs::write(scratch.0.join("input.was"), source).unwrap();
        for (level, flag) in FLAGS.into_iter().enumerate() {
            scratch.compile(Some(flag), selection, &format!("{case}-{level}.wasm"));
        }
    }
    let output = test_process::output(
        Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/support/m6_adapter_runtime.cjs"
            ))
            .arg(&scratch.0),
        None,
        Duration::from_secs(30),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
