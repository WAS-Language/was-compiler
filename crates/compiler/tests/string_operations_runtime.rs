#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{process::Command, time::Duration};
use was_compiler::{Compiler, OptimizationLevel, Type};

fn run(source: &str, script: &str) {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("effect", "env", "effect", vec![Type::Number], Type::Void)
        .unwrap();
    compiler
        .register_host_function(
            "produce",
            "env",
            "produce",
            vec![Type::Number],
            Type::String,
        )
        .unwrap();
    compiler
        .register_host_function("retain", "env", "retain", vec![Type::String], Type::Void)
        .unwrap();

    let script = format!(
        r#"
        const assert = require('node:assert/strict');
        assert.ok(Number(process.versions.node.split('.')[0]) >= 24, 'requires Node.js 24+');
        const {{createAbi}} = require('./runtime/abi.cjs');
        let effect = () => {{}}, produce = () => {{throw new Error('unexpected produce');}},
            retain = () => {{throw new Error('unexpected retain');}};
        const module = new WebAssembly.Module(require('node:fs').readFileSync(0));
        const instance = new WebAssembly.Instance(module, {{env:{{
            effect: n => effect(n), produce: n => produce(n), retain: h => retain(h)
        }}}});
        const e = instance.exports, abi = createAbi(() => instance);
        {script}
        "#
    );

    let mut enabled_bytes = None;
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        let bytes = compiler
            .compile_source(source)
            .unwrap_or_else(|error| panic!("{level:?}: {error:?}"));
        assert_eq!(bytes, compiler.compile_source(source).unwrap());
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled_bytes {
                assert_eq!(&bytes, expected, "enabled-preset byte parity");
            } else {
                enabled_bytes = Some(bytes.clone());
            }
        }
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        let output = test_process::output(
            Command::new("node")
                .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
                .arg("-e")
                .arg(&script),
            Some(&bytes),
            Duration::from_secs(30),
        );
        assert!(
            output.status.success(),
            "{level:?}:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires Node.js 24+"]
fn byte_length_counts_utf8_bytes_including_empty_nul_and_unsigned_payload_bytes() {
    run(
        r#"
        fn bytes(s: string) -> number { return string_byte_len(s); }
        fn literal() -> number { return string_byte_len("éλ🙂"); }
        "#,
        r#"
        for (const [text, expected] of [
            ['', 0], ['ASCII', 5], ['\0', 1], ['a\0b', 3],
            ['é', 2], ['λ', 2], ['🙂', 4], ['éλ🙂', 8],
            ['e\u0301', 3], ['界'.repeat(257), 771], ['x'.repeat(131073), 131073]
        ]) {
            const h = abi.encode('string', text);
            assert.equal(e.bytes(h), expected);
            assert.equal(abi.decode('string', h), text);
        }
        assert.equal(e.literal(), 8);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn concat_reuses_validated_empty_operands_and_copies_nonempty_utf8_without_mutating_aliases() {
    run(
        &r#"
        fn concat(a: string, b: string) { return string_concat(a, b); }
        fn literal() -> string { return "é\0🙂"; }
        fn aliases() -> string[] {
            let value = "é\0🙂";
            let alias = value;
            let saved = [value, alias, "é\0🙂"];
            value = string_concat(value, "!");
            return [saved[0], saved[1], saved[2], alias, value, "é\0🙂"];
        }
        fn replace_slot(a: string[]) { a[0] = string_concat(a[0], "!"); }
        fn same(a: string, b: string) -> bool { return a == b; }
        "#
        .replace("\\0", "\0"),
        r#"
        const empty = abi.encode('string', ''), otherEmpty = abi.encode('string', '');
        for (const [left, right, expected] of [
            ['', '', ''], ['', 'é\0🙂', 'é\0🙂'], ['ASCII', '', 'ASCII'],
            ['a\0', 'λ🙂', 'a\0λ🙂'], ['é', 'e\u0301', 'ée\u0301']
        ]) {
            const a = abi.encode('string', left), b = abi.encode('string', right);
            const result = e.concat(a, b);
            assert.equal(abi.decode('string', result), expected);
            assert.equal(abi.decode('string', a), left);
            assert.equal(abi.decode('string', b), right);
            if (left.length === 0 && right.length !== 0) assert.equal(result, b);
            if (right.length === 0 && left.length !== 0) assert.equal(result, a);
            if (left.length !== 0 && right.length !== 0) {
                assert.notEqual(result, a);
                assert.notEqual(result, b);
                const view = new DataView(e.memory.buffer);
                const data = view.getUint32(result, true), length = view.getUint32(result + 4, true);
                assert.deepEqual(Buffer.from(e.memory.buffer, data, length), Buffer.from(expected, 'utf8'));
            }
        }
        assert.equal(abi.decode('string', e.concat(empty, otherEmpty)), '');
        const literal = e.literal(), values = e.aliases();
        assert.deepEqual(abi.decode('string[]', values),
            ['é\0🙂', 'é\0🙂', 'é\0🙂', 'é\0🙂', 'é\0🙂!', 'é\0🙂']);
        const view = new DataView(e.memory.buffer), data = view.getUint32(values, true);
        for (const index of [0, 1, 2, 3, 5]) {
            assert.equal(view.getUint32(data + 4 * index, true), literal, 'interned literal stays shared');
        }
        e.replace_slot(values);
        assert.deepEqual(abi.decode('string[]', values),
            ['é\0🙂!', 'é\0🙂', 'é\0🙂', 'é\0🙂', 'é\0🙂!', 'é\0🙂']);
        assert.equal(abi.decode('string', literal), 'é\0🙂');
        assert.equal(e.same(e.concat(literal, literal), abi.encode('string', 'é\0🙂é\0🙂')), 1);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn argument_effects_run_once_in_order_and_argument_traps_stop_later_evaluation() {
    run(
        r#"
        fn ordered() -> string {
            return string_concat({ effect(1); produce(10) }, { effect(2); produce(20) });
        }
        fn empty_effects(a: string, b: string) -> string {
            return string_concat({ effect(3); a }, { effect(4); b });
        }
        fn bytes() -> number { return string_byte_len({ effect(5); produce(10) }); }
        fn fail() -> string { let a: number[] = []; let n = a[0]; return "unused"; }
        fn left_trap() -> string {
            return string_concat({ effect(6); fail() }, { effect(7); "right" });
        }
        fn right_trap() -> string {
            return string_concat({ effect(8); let s = string_concat("saved", "!"); retain(s); s },
                                 { effect(9); fail() });
        }
        "#,
        r#"
        const events = [], saved = [];
        effect = n => events.push(n);
        produce = n => { events.push(n); return abi.encode('string', n === 10 ? 'é' : '🙂'); };
        retain = h => saved.push(h);
        assert.equal(abi.decode('string', e.ordered()), 'é🙂');
        assert.deepEqual(events, [1, 10, 2, 20]);
        events.length = 0;
        for (const [a, b, expected] of [['', 'right', 'right'], ['left', '', 'left'], ['', '', '']]) {
            assert.equal(abi.decode('string', e.empty_effects(abi.encode('string', a), abi.encode('string', b))), expected);
        }
        assert.deepEqual(events, [3, 4, 3, 4, 3, 4]);
        events.length = 0;
        assert.equal(e.bytes(), 2);
        assert.deepEqual(events, [5, 10]);
        events.length = 0;
        assert.throws(() => e.left_trap(), WebAssembly.RuntimeError);
        assert.deepEqual(events, [6]);
        events.length = 0;
        assert.throws(() => e.right_trap(), WebAssembly.RuntimeError);
        assert.deepEqual(events, [8, 9]);
        assert.equal(saved.length, 1);
        assert.equal(abi.decode('string', saved[0]), 'saved!');
        // Existing host-return validation stops the second argument immediately.
        events.length = 0;
        produce = n => { events.push(n); return 0; };
        assert.throws(() => e.ordered(), WebAssembly.RuntimeError);
        assert.deepEqual(events, [1, 10]);
        assert.equal(abi.decode('string', saved[0]), 'saved!');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn builtin_validation_after_argument_effects_checks_both_operands_before_empty_reuse() {
    run(
        r#"
        fn concat(a: string, b: string) -> string {
            return string_concat({ effect(1); a }, { effect(2); b });
        }
        fn bytes(a: string) -> number { return string_byte_len({ effect(3); a }); }
        "#,
        r#"
        const events = [];
        // Deliberately violate the trusted-host ABI only after parameter entry checks.
        // This isolates builtin checks instead of merely exercising function entry.
        for (const [left, right, target, field, bad, corruptAt] of [
            ['', 'right', 'a', 0, 0, 1], ['', 'right', 'b', 0, 0, 1],
            ['left', '', 'a', 0, 0, 1], ['left', '', 'b', 0, 0, 1],
            ['', '', 'a', 0, 0, 1], ['', '', 'b', 0, 0, 1],
            ['left', 'right', 'a', 4, 0xffffffff, 1],
            ['left', 'right', 'b', 4, 0x80000000, 1],
            ['', 'right', 'a', 0, 0, 2], ['left', '', 'a', 0, 0, 2],
            ['', 'right', 'b', 0, 0, 2], ['left', '', 'b', 0, 0, 2]
        ]) {
            const a = abi.encode('string', left), b = abi.encode('string', right);
            const address = (target === 'a' ? a : b) + field;
            const original = new DataView(e.memory.buffer).getUint32(address, true);
            events.length = 0;
            effect = n => {
                events.push(n);
                if (n === corruptAt) new DataView(e.memory.buffer).setUint32(address, bad, true);
            };
            assert.throws(() => e.concat(a, b), WebAssembly.RuntimeError);
            assert.deepEqual(events, [1, 2], 'both argument effects precede builtin validation');
            new DataView(e.memory.buffer).setUint32(address, original, true);
            effect = n => events.push(n);
            assert.equal(abi.decode('string', e.concat(a, b)), left + right);
            assert.equal(abi.decode('string', a), left);
            assert.equal(abi.decode('string', b), right);
        }
        for (const [field, bad] of [[0, 0], [0, 0xfffffff8], [4, 0x80000000], [4, 0xffffffff]]) {
            const h = abi.encode('string', 'é');
            const original = new DataView(e.memory.buffer).getUint32(h + field, true);
            events.length = 0;
            effect = n => {
                events.push(n);
                new DataView(e.memory.buffer).setUint32(h + field, bad, true);
            };
            assert.throws(() => e.bytes(h), WebAssembly.RuntimeError);
            assert.deepEqual(events, [3]);
            new DataView(e.memory.buffer).setUint32(h + field, original, true);
            effect = () => {};
            assert.equal(e.bytes(h), 2);
        }
        events.length = 0;
        effect = n => events.push(n);
        const valid = abi.encode('string', 'valid');
        assert.throws(() => e.concat(0, valid), WebAssembly.RuntimeError);
        assert.throws(() => e.concat(valid, 0), WebAssembly.RuntimeError);
        assert.throws(() => e.bytes(0), WebAssembly.RuntimeError);
        assert.deepEqual(events, [], 'entry validation is still before body effects');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn retained_concat_results_survive_growth_reentry_and_caught_traps_with_suspended_arguments() {
    run(
        r#"
        fn concat(a: string, b: string) -> string { return string_concat(a, b); }
        fn publish(s: string) -> string {
            let result = string_concat(s, "!"); retain(result); return result;
        }
        fn fail(s: string) {
            let result = string_concat(s, "?"); retain(result);
            let a: number[] = []; let n = a[0];
        }
        fn outer(s: string) -> string {
            return string_concat(string_concat("outer:", s),
                { effect(1); string_concat(s, ":done") });
        }
        "#,
        r#"
        const retained = [], events = [];
        retain = h => retained.push([h, abi.decode('string', h)]);
        const source = abi.encode('string', 'é🙂');
        const published = e.publish(source);
        assert.equal(abi.decode('string', published), 'é🙂!');
        const oldBuffer = e.memory.buffer;
        effect = n => {
            events.push(n);
            const large = abi.encode('string', '界'.repeat(70000));
            const grown = e.concat(large, large);
            assert.notEqual(e.memory.buffer, oldBuffer, 'fixture must cause actual memory growth');
            assert.equal(abi.decode('string', grown), '界'.repeat(140000));
            retained.push([grown, '界'.repeat(140000)]);
            assert.throws(() => e.fail(source), WebAssembly.RuntimeError);
            assert.equal(abi.decode('string', retained.at(-1)[0]), 'é🙂?');
            const inner = e.publish(abi.encode('string', 'inner'));
            assert.equal(abi.decode('string', inner), 'inner!');
            for (const [h, expected] of retained) assert.equal(abi.decode('string', h), expected);
        };
        const outer = e.outer(source);
        assert.deepEqual(events, [1]);
        assert.equal(abi.decode('string', outer), 'outer:é🙂é🙂:done');
        assert.equal(abi.decode('string', source), 'é🙂');
        assert.equal(abi.decode('string', published), 'é🙂!');
        assert.deepEqual(retained.map(([, text]) => text),
            ['é🙂!', '界'.repeat(140000), 'é🙂?', 'inner!']);
        // Subsequent allocations and a later uncaught invocation trap cannot retire prior results.
        e.concat(abi.encode('string', 'x'.repeat(300000)), abi.encode('string', 'y'.repeat(300000)));
        assert.throws(() => e.fail(source), WebAssembly.RuntimeError);
        for (const [h, expected] of retained) assert.equal(abi.decode('string', h), expected);
        assert.equal(abi.decode('string', outer), 'outer:é🙂é🙂:done');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn nested_operations_preserve_live_stack_prefixes_captures_and_recursive_invocation_state() {
    run(
        r#"
        fn sum(a: number, b: number, c: number) -> number { return a + b + c; }
        fn nested(s: string) -> number {
            return 100 + sum(7, string_byte_len(string_concat("é", string_concat(s, "🙂"))),
                3 + string_byte_len(string_concat("x", "λ")));
        }
        fn arrays(s: string) -> string[] {
            return ["prefix", string_concat("left:", string_concat(s, ":right")),
                string_concat("tail", "!")];
        }
        fn captured(s: string) {
            let alias = s;
            let build = () => string_concat(alias, "!");
            s = string_concat(s, "changed");
            return string_concat(build(), alias);
        }
        fn recursive(depth: number, s: string) -> string {
            if depth == 0 { return string_concat(s, "!"); }
            return string_concat(string_concat(s, "["),
                string_concat(recursive(depth - 1, s), "]"));
        }
        fn branch(s: string, yes: bool) -> string {
            return string_concat("pre:", if yes { string_concat(s, "!") }
                else { string_concat("?", s) });
        }
        "#,
        r#"
        const s = abi.encode('string', 'λ');
        assert.equal(e.nested(s), 121);
        assert.deepEqual(abi.decode('string[]', e.arrays(s)), ['prefix', 'left:λ:right', 'tail!']);
        assert.equal(abi.decode('string', e.captured(s)), 'λ!λ');
        assert.equal(abi.decode('string', e.recursive(3, s)), 'λ[λ[λ[λ!]]]');
        assert.equal(abi.decode('string', e.branch(s, 1)), 'pre:λ!');
        assert.equal(abi.decode('string', e.branch(s, 0)), 'pre:?λ');
        assert.equal(abi.decode('string', s), 'λ');
        "#,
    );
}
