#[path = "../../../tests/support/m6_adapter_cases.rs"]
mod cases;
#[path = "../../../tests/support/process.rs"]
mod test_process;

use cases::{CASES, PRIVATE_HOST_HINT, SOURCE, Selection};
use std::{path::Path, process::Command, time::Duration};
use was_compiler::{Type, diagnostic};

fn field(input: &mut Vec<u8>, bytes: &[u8]) {
    input.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
    input.extend_from_slice(bytes);
}

#[test]
#[ignore = "requires Node.js and freshly rebuilt browser bindings in pkg"]
fn rebuilt_browser_matches_core_and_executes_range_modules_at_every_preset() {
    let mut input = Vec::new();
    field(&mut input, CASES.len().to_string().as_bytes());

    for ((case, source, selection), native) in CASES.into_iter().zip(cases::core_matrix()) {
        field(&mut input, case.as_bytes());
        field(&mut input, source.as_bytes());
        let selection = match selection {
            Selection::Selected => cases::EXPORTS.join(","),
            Selection::None => String::new(),
            Selection::All => "*".to_owned(),
        };
        field(&mut input, selection.as_bytes());
        for bytes in native {
            field(&mut input, &bytes);
            field(
                &mut input,
                wasmprinter::print_bytes(&bytes).unwrap().as_bytes(),
            );
        }
    }

    let diagnostics = cases::diagnostics();
    field(&mut input, diagnostics.len().to_string().as_bytes());
    for (source, baseline) in diagnostics {
        let mut expected = diagnostic::render("<browser>", source, &baseline);
        if source == SOURCE {
            expected.push_str("\nSelect only compatible functions with setExports([\"main\"]), or use exportNone() to keep all user functions internal.");
        }
        field(&mut input, source.as_bytes());
        field(&mut input, expected.as_bytes());
    }
    let mut core = cases::compiler();
    for (params, result) in [(vec![Type::Range], Type::Void), (vec![], Type::Range)] {
        let error = core
            .register_host_function("bad", "range", "for", params, result)
            .unwrap_err();
        field(
            &mut input,
            format!("host registration error: {error}{PRIVATE_HOST_HINT}").as_bytes(),
        );
    }

    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let output = test_process::output(
        Command::new("node")
            .arg(root.join("tests/support/m6_adapter_browser.cjs"))
            .arg(root.join("pkg")),
        Some(&input),
        Duration::from_secs(30),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
