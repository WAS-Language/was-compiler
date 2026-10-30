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

#[test]
#[ignore = "requires Node.js 24+"]
fn slice_bytes_checks_numeric_edges_and_every_utf8_endpoint_including_empty_interiors() {
    run(
        r#"
        fn slice(s: string, start: number, end: number) -> string {
            return string_slice_bytes(s, start, end);
        }
        "#,
        r#"
        for (const text of ['', 'ASCII', '\0', 'a\0b', 'éλ🙂', 'Aé\0🙂界Z', 'e\u0301']) {
            const source = abi.encode('string', text), bytes = Buffer.from(text, 'utf8');
            const boundaries = new Set([0]);
            let offset = 0;
            for (const scalar of text) {
                offset += Buffer.byteLength(scalar, 'utf8');
                boundaries.add(offset);
            }
            for (let start = 0; start <= bytes.length; start++) {
                for (let end = 0; end <= bytes.length; end++) {
                    if (start > end || !boundaries.has(start) || !boundaries.has(end)) {
                        assert.throws(() => e.slice(source, start, end), WebAssembly.RuntimeError,
                            JSON.stringify([text, start, end]));
                    } else {
                        const result = e.slice(source, start, end);
                        assert.equal(abi.decode('string', result), bytes.subarray(start, end).toString('utf8'));
                        if (start === 0 && end === bytes.length) assert.equal(result, source);
                    }
                }
            }
            assert.equal(e.slice(source, -0, bytes.length), source);
            assert.equal(abi.decode('string', e.slice(source, -0, -0)), '');
            assert.equal(abi.decode('string', source), text);
        }
        const source = abi.encode('string', 'abcd');
        const edges = [NaN, Infinity, -Infinity, -Number.MAX_VALUE, Number.MAX_VALUE,
            -Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER, -1, -0.5, 0.5, 3.5,
            -Number.MIN_VALUE, Number.MIN_VALUE, 5, 2 ** 31, 2 ** 32,
            2 ** 32 + 1, 2 ** 53, -0, 0, 1, 2, 3, 4];
        for (const start of edges) {
            for (const end of edges) {
                const valid = Number.isFinite(start) && Number.isInteger(start) && start >= 0 &&
                    Number.isFinite(end) && Number.isInteger(end) && end >= start && end <= 4;
                if (valid) assert.equal(abi.decode('string', e.slice(source, start, end)), 'abcd'.slice(start, end));
                else assert.throws(() => e.slice(source, start, end), WebAssembly.RuntimeError,
                    `start=${start}, end=${end}`);
            }
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn slice_bytes_accepts_memory_end_endpoints_and_does_not_scan_utf8_payloads() {
    run(
        r#"
        fn slice(s: string, start: number, end: number) -> string {
            return string_slice_bytes(s, start, end);
        }
        "#,
        r#"
        const h = abi.encode('string', 'descriptor');
        // Keep the descriptor in allocated storage, but put its payload at the exact
        // memory end. Empty end slices and full reuse must not load data[length].
        const limit = e.memory.buffer.byteLength;
        const bytes = Buffer.from('Aé🙂', 'utf8'), pointer = limit - bytes.length;
        new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
        let view = new DataView(e.memory.buffer);
        view.setUint32(h, pointer, true);
        view.setUint32(h + 4, bytes.length, true);
        assert.equal(e.slice(h, 0, bytes.length), h);
        assert.equal(abi.decode('string', e.slice(h, bytes.length, bytes.length)), '');
        assert.equal(abi.decode('string', e.slice(h, 1, bytes.length)), 'é🙂');
        assert.equal(abi.decode('string', e.slice(h, 3, bytes.length)), '🙂');
        assert.throws(() => e.slice(h, 4, bytes.length), WebAssembly.RuntimeError);
        assert.throws(() => e.slice(h, bytes.length, bytes.length + 1), WebAssembly.RuntimeError);
        // A zero-length descriptor can point one past the final byte as well.
        view = new DataView(e.memory.buffer);
        view.setUint32(h, e.memory.buffer.byteLength, true);
        view.setUint32(h + 4, 0, true);
        assert.equal(e.slice(h, -0, 0), h);

        // Deliberately violate the trusted-host UTF-8 obligation. Only structural
        // validity and endpoint continuation bytes are checked, not interior UTF-8.
        const raw = abi.encode('string', 'abcdef');
        view = new DataView(e.memory.buffer);
        const data = view.getUint32(raw, true);
        new Uint8Array(e.memory.buffer, data, 6).set([0x61, 0x62, 0xff, 0x63, 0x64, 0x65]);
        assert.equal(e.slice(raw, 0, 6), raw);
        const copied = e.slice(raw, 1, 5);
        view = new DataView(e.memory.buffer);
        assert.equal(view.getUint32(copied + 4, true), 4);
        assert.deepEqual([...new Uint8Array(e.memory.buffer, view.getUint32(copied, true), 4)],
            [0x62, 0xff, 0x63, 0x64]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn slice_bytes_reuses_full_ranges_and_copies_proper_ranges_without_changing_aliases() {
    run(
        r#"
        fn slice(s: string, start: number, end: number) -> string {
            return string_slice_bytes(s, start, end);
        }
        fn aliases() -> string[] {
            let source = "Aé🙂Z";
            let alias = source;
            let saved = [source, alias];
            let part = string_slice_bytes(source, 1, 7);
            source = string_slice_bytes(source, 3, 7);
            return [saved[0], saved[1], alias, source, part, "Aé🙂Z"];
        }
        fn replace_slot(a: string[]) { a[0] = string_slice_bytes(a[0], 1, 7); }
        fn nested(s: string) -> number {
            let part = () => string_slice_bytes(s, 1, 7);
            return 100 + string_byte_len(string_concat("x", string_slice_bytes(part(), 2, 6)));
        }
        "#,
        r#"
        const source = abi.encode('string', 'Aé🙂Z');
        assert.equal(e.slice(source, 0, 8), source);
        const initial = new DataView(e.memory.buffer), sourceData = initial.getUint32(source, true);
        const copied = e.slice(source, 1, 7);
        const view = new DataView(e.memory.buffer), copiedData = view.getUint32(copied, true);
        assert.notEqual(copied, source);
        assert.equal(view.getUint32(copied + 4, true), 6);
        assert.ok(copiedData + 6 <= sourceData || sourceData + 8 <= copiedData,
            'proper slice owns a disjoint copied payload, not an interior view');
        assert.deepEqual(Buffer.from(e.memory.buffer, copiedData, 6), Buffer.from('é🙂'));
        assert.equal(abi.decode('string', source), 'Aé🙂Z');
        const values = e.aliases();
        assert.deepEqual(abi.decode('string[]', values), ['Aé🙂Z', 'Aé🙂Z', 'Aé🙂Z', '🙂', 'é🙂', 'Aé🙂Z']);
        const slots = new DataView(e.memory.buffer), data = slots.getUint32(values, true);
        const literal = slots.getUint32(data, true);
        for (const index of [1, 2, 5]) assert.equal(slots.getUint32(data + index * 4, true), literal);
        e.replace_slot(values);
        assert.deepEqual(abi.decode('string[]', values), ['é🙂', 'Aé🙂Z', 'Aé🙂Z', '🙂', 'é🙂', 'Aé🙂Z']);
        assert.equal(abi.decode('string', literal), 'Aé🙂Z');
        assert.equal(e.nested(source), 105);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn slice_bytes_evaluates_arguments_once_before_checks_but_preserves_argument_trap_boundaries() {
    run(
        r#"
        fn ordered(start: number, end: number) -> string {
            return string_slice_bytes({ effect(1); produce(10) },
                { effect(2); start }, { effect(3); end });
        }
        fn fail_number() -> number { let a: number[] = []; return a[0]; }
        fn source_trap() -> string {
            return string_slice_bytes({ effect(4); let n = fail_number(); "unused" },
                { effect(5); 0 }, { effect(6); 0 });
        }
        fn start_trap(s: string) -> string {
            return string_slice_bytes({ effect(7); s }, { effect(8); fail_number() }, { effect(9); 0 });
        }
        fn end_trap(s: string) -> string {
            return string_slice_bytes({ effect(11); let part = string_slice_bytes(s, 1, 7); retain(part); s },
                { effect(12); 1 }, { effect(13); fail_number() });
        }
        "#,
        r#"
        const source = abi.encode('string', 'Aé🙂Z'), events = [], retained = [];
        effect = n => events.push(n);
        produce = n => { events.push(n); return source; };
        retain = h => retained.push(h);
        for (const [start, end, expected] of [[1, 7, 'é🙂'], [0, 8, 'Aé🙂Z'], [3, 3, '']]) {
            events.length = 0;
            assert.equal(abi.decode('string', e.ordered(start, end)), expected);
            assert.deepEqual(events, [1, 10, 2, 3]);
        }
        for (const [start, end] of [[NaN, 8], [0, Infinity], [-1, 8], [7, 1], [0, 9], [2, 2], [1, 4]]) {
            events.length = 0;
            assert.throws(() => e.ordered(start, end), WebAssembly.RuntimeError);
            assert.deepEqual(events, [1, 10, 2, 3], 'numeric and scalar-boundary checks follow all arguments');
        }
        for (const [call, expected] of [
            [() => e.source_trap(), [4]], [() => e.start_trap(source), [7, 8]],
            [() => e.end_trap(source), [11, 12, 13]]
        ]) {
            events.length = 0;
            assert.throws(call, WebAssembly.RuntimeError);
            assert.deepEqual(events, expected);
        }
        assert.equal(retained.length, 1);
        assert.equal(abi.decode('string', retained[0]), 'é🙂');
        events.length = 0;
        produce = n => { events.push(n); return 0; };
        assert.throws(() => e.ordered(0, 0), WebAssembly.RuntimeError);
        assert.deepEqual(events, [1, 10], 'host-return validation still precedes later arguments');
        assert.equal(abi.decode('string', retained[0]), 'é🙂');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn slice_bytes_revalidates_post_entry_descriptors_after_effects_even_before_full_or_empty_reuse() {
    run(
        r#"
        fn slice(s: string, start: number, end: number) -> string {
            return string_slice_bytes({ effect(1); s }, { effect(2); start }, { effect(3); end });
        }
        "#,
        r#"
        const events = [];
        for (const text of ['', 'Aé🙂Z']) {
            const h = abi.encode('string', text), length = Buffer.byteLength(text);
            const ranges = length === 0 ? [[0, 0]] : [[0, length], [0, 0], [length, length], [1, 7]];
            for (const [start, end] of ranges) {
                for (const corruptAt of [1, 2, 3]) {
                    for (const [field, bad] of [[0, 0], [0, 0xfffffff8], [4, 0x80000000], [4, 0xffffffff]]) {
                        const original = new DataView(e.memory.buffer).getUint32(h + field, true);
                        events.length = 0;
                        effect = n => {
                            events.push(n);
                            if (n === corruptAt) new DataView(e.memory.buffer).setUint32(h + field, bad, true);
                        };
                        assert.throws(() => e.slice(h, start, end), WebAssembly.RuntimeError);
                        assert.deepEqual(events, [1, 2, 3], 'no builtin validation between argument effects');
                        new DataView(e.memory.buffer).setUint32(h + field, original, true);
                        effect = () => {};
                        const result = e.slice(h, start, end);
                        assert.equal(abi.decode('string', result), Buffer.from(text).subarray(start, end).toString());
                        if (start === 0 && end === length) assert.equal(result, h);
                    }
                }
            }
        }
        events.length = 0;
        effect = n => events.push(n);
        assert.throws(() => e.slice(0, 0, 0), WebAssembly.RuntimeError);
        const invalid = abi.encode('string', 'entry');
        new DataView(e.memory.buffer).setUint32(invalid, 0, true);
        assert.throws(() => e.slice(invalid, 0, 5), WebAssembly.RuntimeError);
        assert.deepEqual(events, [], 'parameter-entry validation still precedes body effects');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn retained_slices_survive_growth_reentry_and_caught_traps_with_suspended_arguments() {
    run(
        r#"
        fn slice(s: string, start: number, end: number) -> string {
            return string_slice_bytes(s, start, end);
        }
        fn publish(s: string) -> string {
            let part = string_slice_bytes(s, 1, string_byte_len(s) - 1);
            retain(part); return part;
        }
        fn fail(s: string) {
            let part = string_slice_bytes(s, 1, 7); retain(part);
            let invalid = string_slice_bytes(s, 2, 2);
        }
        fn outer(s: string) -> string {
            return string_concat("outer:", string_slice_bytes(
                string_concat("!", s), { effect(1); 2 }, { effect(2); 8 }));
        }
        "#,
        r#"
        const retained = [], events = [], source = abi.encode('string', 'Aé🙂Z');
        retain = h => retained.push([h, abi.decode('string', h)]);
        const published = e.publish(source);
        const oldBuffer = e.memory.buffer;
        effect = n => {
            events.push(n);
            if (n !== 1) return;
            const text = 'A' + '界'.repeat(90000) + 'Z';
            const large = abi.encode('string', text);
            // Consume current headroom so the proper slice itself must grow memory.
            const marker = e.__alloc(8) >>> 0;
            const remaining = e.memory.buffer.byteLength - (marker + 8);
            if (remaining > 0) e.__alloc(remaining);
            const beforeSlice = e.memory.buffer;
            const grown = e.publish(large);
            assert.notEqual(e.memory.buffer, beforeSlice, 'proper slice allocation itself grows memory');
            assert.notEqual(e.memory.buffer, oldBuffer, 'reentry fixture causes actual memory growth');
            assert.equal(abi.decode('string', grown), '界'.repeat(90000));
            assert.throws(() => e.fail(source), WebAssembly.RuntimeError);
            const inner = e.publish(abi.encode('string', 'BλC'));
            assert.equal(abi.decode('string', inner), 'λ');
            for (const [h, expected] of retained) assert.equal(abi.decode('string', h), expected);
        };
        const outer = e.outer(source);
        assert.deepEqual(events, [1, 2]);
        assert.equal(abi.decode('string', outer), 'outer:é🙂');
        assert.equal(abi.decode('string', source), 'Aé🙂Z');
        assert.equal(abi.decode('string', published), 'é🙂');
        assert.deepEqual(retained.map(([, text]) => text), ['é🙂', '界'.repeat(90000), 'é🙂', 'λ']);
        assert.throws(() => e.fail(source), WebAssembly.RuntimeError);
        e.publish(abi.encode('string', 'X' + 'x'.repeat(400000) + 'Y'));
        for (const [h, expected] of retained) assert.equal(abi.decode('string', h), expected);
        assert.equal(abi.decode('string', outer), 'outer:é🙂');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn replace_all_matches_literal_nonoverlapping_utf8_and_copies_only_changed_nonempty_results() {
    run(
        r#"
        fn replace(s: string, n: string, r: string) -> string {
            return string_replace_all(s, n, r);
        }
        fn aliases() -> string[] {
            let source = "aaaé🙂";
            let alias = source;
            let saved = [source, alias];
            source = string_replace_all(source, "aa", "b");
            return [saved[0], saved[1], alias, source, "aaaé🙂"];
        }
        fn replace_slot(a: string[]) { a[0] = string_replace_all(a[0], "aa", "b"); }
        "#,
        r#"
        for (const [text, needle, replacement, expected] of [
            ['aaa', 'aa', 'b', 'ba'], ['aaaaa', 'aa', 'b', 'bba'],
            ['abcabc', 'abc', '', ''], ['abcabcZ', 'abc', '', 'Z'],
            ['aaa', 'a', 'aa', 'aaaaaa'], ['aba', 'a', 'aba', 'abababa'],
            ['AaA', 'a', 'x', 'AxA'], ['abc', 'abcd', 'x', 'abc'],
            ['', 'a', 'x', ''], ['abc', 'z', '', 'abc'],
            ['a\0b\0', '\0', '🙂\0', 'a🙂\0b🙂\0'],
            ['éλ🙂éλ🙂', 'λ🙂', '界', 'é界é界'],
            ['é/e\u0301/É', 'é', 'λ', 'λ/e\u0301/É'],
            ['a.a.a', '.', '$&$`$\'', 'a$&$`$\'a$&$`$\'a'],
            ['aaa', 'aa', 'aa', 'aaa'], ['é🙂', 'é', 'é', 'é🙂']
        ]) {
            const handles = [text, needle, replacement].map(s => abi.encode('string', s));
            const [s, n, r] = handles;
            assert.notEqual(n, r, 'equal contents still have distinct input descriptors');
            const result = e.replace(s, n, r);
            assert.equal(abi.decode('string', result), expected);
            assert.equal(e.replace(s, n, r) === s, result === s, 'reuse policy is deterministic');
            for (let i = 0; i < handles.length; i++) {
                assert.equal(abi.decode('string', handles[i]), [text, needle, replacement][i]);
            }
            if (!text.includes(needle) || needle === replacement) {
                assert.equal(result, s, 'no-match and content-identical replacement reuse source');
            } else if (expected.length !== 0) {
                const view = new DataView(e.memory.buffer);
                const data = view.getUint32(result, true), length = view.getUint32(result + 4, true);
                assert.equal(length, Buffer.byteLength(expected));
                assert.deepEqual(Buffer.from(e.memory.buffer, data, length), Buffer.from(expected));
                for (const h of handles) {
                    assert.notEqual(result, h);
                    const inputData = view.getUint32(h, true), inputLength = view.getUint32(h + 4, true);
                    assert.ok(data + length <= inputData || inputData + inputLength <= data,
                        'changed nonempty result owns a disjoint copied payload');
                }
            }
        }
        const values = e.aliases();
        assert.deepEqual(abi.decode('string[]', values), ['aaaé🙂', 'aaaé🙂', 'aaaé🙂', 'baé🙂', 'aaaé🙂']);
        const view = new DataView(e.memory.buffer), slots = view.getUint32(values, true);
        const literal = view.getUint32(slots, true);
        for (const i of [1, 2, 4]) assert.equal(view.getUint32(slots + 4 * i, true), literal);
        e.replace_slot(values);
        assert.deepEqual(abi.decode('string[]', values), ['baé🙂', 'aaaé🙂', 'aaaé🙂', 'baé🙂', 'aaaé🙂']);
        assert.equal(abi.decode('string', literal), 'aaaé🙂');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn replace_all_runs_arguments_once_before_empty_needle_checks_and_retains_effects_after_traps() {
    run(
        r#"
        fn ordered() -> string {
            return string_replace_all({ effect(1); produce(10) },
                { effect(2); produce(20) }, { effect(3); produce(30) });
        }
        fn fail() -> string { let a: string[] = []; return a[0]; }
        fn source_trap() -> string {
            return string_replace_all({ effect(4); fail() }, { effect(5); "a" }, { effect(6); "b" });
        }
        fn needle_trap(s: string) -> string {
            return string_replace_all({ effect(7); s }, { effect(8); fail() }, { effect(9); "b" });
        }
        fn replacement_trap(s: string) -> string {
            return string_replace_all({ effect(11); let saved = string_replace_all(s, "a", "λ");
                retain(saved); s }, { effect(12); "" }, { effect(13); fail() });
        }
        fn empty_needle(s: string) -> string {
            return string_replace_all({ effect(14); s }, { effect(15); "" },
                { effect(16); let saved = string_replace_all(s, "a", "🙂"); retain(saved); "x" });
        }
        "#,
        r#"
        const events = [], retained = [], source = abi.encode('string', 'aba');
        effect = n => events.push(n);
        retain = h => retained.push([h, abi.decode('string', h)]);
        for (const [texts, expected] of [
            [['aba', 'a', 'λ'], 'λbλ'], [['aba', 'z', 'x'], 'aba'],
            [['aba', 'a', 'a'], 'aba'], [['', 'a', 'x'], '']
        ]) {
            events.length = 0;
            produce = n => { events.push(n); return abi.encode('string', texts[n / 10 - 1]); };
            assert.equal(abi.decode('string', e.ordered()), expected);
            assert.deepEqual(events, [1, 10, 2, 20, 3, 30]);
        }
        for (const text of ['', 'aba']) {
            events.length = 0;
            produce = n => { events.push(n); return abi.encode('string', n === 10 ? text : ''); };
            assert.throws(() => e.ordered(), WebAssembly.RuntimeError);
            assert.deepEqual(events, [1, 10, 2, 20, 3, 30], 'empty needle checked after replacement effects');
        }
        for (const [call, expected] of [
            [() => e.source_trap(), [4]], [() => e.needle_trap(source), [7, 8]],
            [() => e.replacement_trap(source), [11, 12, 13]],
            [() => e.empty_needle(source), [14, 15, 16]]
        ]) {
            events.length = 0;
            assert.throws(call, WebAssembly.RuntimeError);
            assert.deepEqual(events, expected);
        }
        assert.deepEqual(retained.map(([, text]) => text), ['λbλ', '🙂b🙂']);
        for (const badAt of [10, 20, 30]) {
            events.length = 0;
            produce = n => { events.push(n); return n === badAt ? 0 : abi.encode('string', 'a'); };
            assert.throws(() => e.ordered(), WebAssembly.RuntimeError);
            assert.deepEqual(events, [1, 10, 2, 20, 3, 30].slice(0, badAt / 5),
                'existing host-return validation stops later argument evaluation');
        }
        for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        assert.equal(abi.decode('string', source), 'aba');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn replace_all_revalidates_even_unused_operands_after_all_argument_effects() {
    run(
        r#"
        fn replace(s: string, n: string, r: string) -> string {
            return string_replace_all({ effect(1); s }, { effect(2); n }, { effect(3); r });
        }
        "#,
        r#"
        const events = [];
        // Corrupt only after entry so fast-path omissions cannot hide behind parameter checks.
        for (const texts of [
            ['', 'a', 'x'], ['abc', 'z', 'x'], ['aaa', 'aa', 'aa'],
            ['aaa', 'a', ''], ['aaa', '', ''], ['', '', 'x'], ['aaa', 'a', 'λ']
        ]) {
            const handles = texts.map(s => abi.encode('string', s));
            for (const target of handles) {
                for (const corruptAt of [1, 2, 3]) {
                    for (const [field, bad] of [[0, 0], [0, 0xfffffff8], [4, 0x80000000], [4, 0xffffffff]]) {
                        const address = target + field;
                        const original = new DataView(e.memory.buffer).getUint32(address, true);
                        events.length = 0;
                        effect = n => {
                            events.push(n);
                            if (n === corruptAt) new DataView(e.memory.buffer).setUint32(address, bad, true);
                        };
                        assert.throws(() => e.replace(...handles), WebAssembly.RuntimeError);
                        assert.deepEqual(events, [1, 2, 3]);
                        new DataView(e.memory.buffer).setUint32(address, original, true);
                    }
                }
            }
            effect = () => {};
            if (texts[1] === '') {
                assert.throws(() => e.replace(...handles), WebAssembly.RuntimeError);
            } else {
                assert.equal(abi.decode('string', e.replace(...handles)), texts[0].split(texts[1]).join(texts[2]));
            }
            for (let i = 0; i < handles.length; i++) assert.equal(abi.decode('string', handles[i]), texts[i]);
        }
        effect = n => events.push(n);
        const valid = abi.encode('string', 'a');
        events.length = 0;
        for (let i = 0; i < 3; i++) {
            const handles = [valid, valid, valid]; handles[i] = 0;
            assert.throws(() => e.replace(...handles), WebAssembly.RuntimeError);
        }
        assert.deepEqual(events, [], 'entry validation remains before body effects');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn replace_all_results_survive_growth_reentry_and_traps_with_suspended_arguments() {
    run(
        r#"
        fn replace(s: string, n: string, r: string) -> string { return string_replace_all(s, n, r); }
        fn publish(s: string) -> string {
            let result = string_replace_all(s, "a", "🙂"); retain(result); return result;
        }
        fn fail(s: string) {
            let result = string_replace_all(s, "a", "λ"); retain(result);
            let invalid = string_replace_all(s, "", "unused");
        }
        fn outer(s: string) -> string {
            return string_concat("outer:", string_replace_all(
                string_replace_all(s, "a", "é"), { effect(1); "é" }, { effect(2); "🙂" }));
        }
        "#,
        r#"
        const retained = [], events = [], source = abi.encode('string', 'aba');
        retain = h => retained.push([h, abi.decode('string', h)]);
        const published = e.publish(source), oldBuffer = e.memory.buffer;
        effect = n => {
            events.push(n);
            if (n !== 1) return;
            const large = abi.encode('string', 'a'.repeat(90000));
            const marker = e.__alloc(8) >>> 0;
            const remaining = e.memory.buffer.byteLength - (marker + 8);
            if (remaining > 0) e.__alloc(remaining);
            const beforeReplace = e.memory.buffer;
            const grown = e.publish(large);
            assert.notEqual(e.memory.buffer, beforeReplace, 'replacement allocation itself grows memory');
            assert.notEqual(e.memory.buffer, oldBuffer);
            assert.equal(abi.decode('string', grown), '🙂'.repeat(90000));
            assert.throws(() => e.fail(source), WebAssembly.RuntimeError);
            assert.equal(abi.decode('string', e.publish(abi.encode('string', 'aλ'))), '🙂λ');
            for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        };
        const outer = e.outer(source);
        assert.deepEqual(events, [1, 2]);
        assert.equal(abi.decode('string', outer), 'outer:🙂b🙂');
        assert.equal(abi.decode('string', published), '🙂b🙂');
        assert.equal(abi.decode('string', source), 'aba');
        assert.deepEqual(retained.map(([, text]) => text),
            ['🙂b🙂', '🙂'.repeat(90000), 'λbλ', '🙂λ']);
        assert.throws(() => e.fail(source), WebAssembly.RuntimeError);
        e.publish(abi.encode('string', 'a'.repeat(150000)));
        for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        assert.equal(abi.decode('string', outer), 'outer:🙂b🙂');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn replace_all_preserves_live_stack_captures_recursion_and_bounded_repetitive_searches() {
    run(
        r#"
        fn replace(s: string, n: string, r: string) -> string { return string_replace_all(s, n, r); }
        fn sum(a: number, b: number, c: number) -> number { return a + b + c; }
        fn nested(s: string) -> number {
            let build = () => string_replace_all(s, "aa", "🙂");
            return 100 + sum(7, string_byte_len(string_replace_all(build(), "🙂", "λ")),
                3 + string_byte_len(string_replace_all("aaa", "aa", "b")));
        }
        fn arrays(s: string) -> string[] {
            return ["prefix", string_replace_all(s, "aa", "b"), string_replace_all(s, "a", "aa")];
        }
        fn recursive(depth: number, s: string) -> string {
            if depth == 0 { return string_replace_all(s, "aa", "b"); }
            return string_concat(string_replace_all(s, "aa", "λ"), recursive(depth - 1, s));
        }
        "#,
        r#"
        const source = abi.encode('string', 'aaa');
        assert.equal(e.nested(source), 115);
        assert.deepEqual(abi.decode('string[]', e.arrays(source)), ['prefix', 'ba', 'aaaaaa']);
        assert.equal(abi.decode('string', e.recursive(3, source)), 'λaλaλaba');
        // Bounded adversarial near-matches, periodic overlaps, dense expansion and deletion.
        // These are correctness/deadline checks, not a linear-time search or timing claim.
        for (const [text, needle, replacement, expected] of [
            ['a'.repeat(8192), 'a'.repeat(127) + 'b', 'x', 'a'.repeat(8192)],
            ['a'.repeat(8192) + 'b', 'a'.repeat(127) + 'b', 'λ', 'a'.repeat(8065) + 'λ'],
            ['ab'.repeat(4096), 'abababa', 'x', 'xb'.repeat(1024)],
            ['a'.repeat(8193), 'aa', '', 'a'],
            ['a'.repeat(4096), 'a', 'aa🙂', 'aa🙂'.repeat(4096)],
            ['界'.repeat(4097), '界界', '🙂', '🙂'.repeat(2048) + '界']
        ]) {
            const handles = [text, needle, replacement].map(s => abi.encode('string', s));
            const result = e.replace(...handles);
            assert.equal(abi.decode('string', result), expected);
            assert.equal(abi.decode('string', handles[0]), text);
            if (!text.includes(needle)) assert.equal(result, handles[0]);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn replace_all_checked_expansion_traps_before_allocation_without_large_memory() {
    run(
        r#"
        fn replace(s: string, n: string, r: string) -> string {
            return string_replace_all(s, n, r);
        }
        "#,
        r#"
        const needle = abi.encode('string', 'a');
        const small = abi.encode('string', 'aba'), item = abi.encode('string', 'λ');
        const retained = e.replace(small, needle, item);
        assert.equal(abi.decode('string', retained), 'λbλ');
        for (const [sourceLength, replacementLength, expectedSize] of [
            [65536, 65536, 0x100000000n],
            // Payload fits u32, but adding the eight-byte descriptor cannot fit memory32.
            [65535, 65537, 0xffffffffn]
        ]) {
            assert.equal(BigInt(sourceLength) * BigInt(replacementLength), expectedSize);
            const sourceText = 'a'.repeat(sourceLength), replacementText = 'b'.repeat(replacementLength);
            const source = abi.encode('string', sourceText);
            const replacement = abi.encode('string', replacementText);
            const before = e.__alloc(8) >>> 0, buffer = e.memory.buffer;
            assert.throws(() => e.replace(source, needle, replacement), WebAssembly.RuntimeError);
            assert.equal(e.memory.buffer, buffer, 'overflow must not attempt memory growth');
            const after = e.__alloc(8) >>> 0;
            assert.equal(after, before + 8, 'overflow must not advance the heap cursor');
            assert.equal(e.memory.buffer, buffer);
            assert.equal(abi.decode('string', source), sourceText);
            assert.equal(abi.decode('string', needle), 'a');
            assert.equal(abi.decode('string', replacement), replacementText);
            assert.equal(abi.decode('string', retained), 'λbλ');
            assert.equal(abi.decode('string', e.replace(small, needle, item)), 'λbλ',
                'a later valid allocation and replacement still succeeds');
            assert.equal(abi.decode('string', small), 'aba');
            assert.equal(abi.decode('string', item), 'λ');
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_captures_parts_once_and_observes_separator_mutation_resize_and_rebinding() {
    run(
        r#"
        fn joined() -> string[] {
            let parts = resizable(["old", "tail"]);
            let alias = parts;
            let result = string_join({ effect(1); parts }, {
                effect(2);
                parts[0] = "é";
                parts.reserve(64);
                parts.push("🙂");
                parts = ["redirected"];
                effect(3);
                ","
            });
            return [result, alias[0], alias[1], alias[2], parts[0]];
        }
        "#,
        r#"
        const events = [];
        effect = n => events.push(n);
        assert.deepEqual(abi.decode('string[]', e.joined()),
            ['é,tail,🙂', 'é', 'tail', '🙂', 'redirected']);
        assert.deepEqual(events, [1, 2, 3]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_handles_fixed_resizable_empty_utf8_and_nul_parts_with_validated_reuse_and_fresh_copies() {
    run(
        &r#"
        fn join(parts: string[], separator: string) -> string { return string_join(parts, separator); }
        fn movable(parts: string[]) -> string[] { return resizable(parts); }
        fn contextual() -> string { return string_join([], "unused"); }
        fn literal() -> string { return string_join(["é", "\0", "🙂"], "λ"); }
        fn set_first(parts: string[], value: string) { parts[0] = value; }
        "#
        .replace("\\0", "\0"),
        r#"
        const cases = [
            [[], '', ''], [[], 'unused', ''], [[''], ',', ''], [['é\0🙂'], 'unused', 'é\0🙂'],
            [['', ''], '', ''], [['', ''], ',', ','], [['a', '', 'b'], ',', 'a,,b'],
            [['', 'a', ''], 'λ', 'λaλ'], [['é', '🙂', 'e\u0301'], '\0λ', 'é\0λ🙂\0λe\u0301'],
            [['a\0', 'λ🙂'], '', 'a\0λ🙂'], [['é', '', '', '🙂'], '界', 'é界界界🙂'],
            [['x'.repeat(70001), 'λ'.repeat(8193)], '🙂', 'x'.repeat(70001) + '🙂' + 'λ'.repeat(8193)],
            [Array(4097).fill(''), 'λ', 'λ'.repeat(4096)],
            [Array(4097).fill('a'), '', 'a'.repeat(4097)]
        ];
        for (const [texts, separatorText, expected] of cases) {
            const fixed = abi.encode('string[]', texts), separator = abi.encode('string', separatorText);
            for (const parts of [fixed, e.movable(fixed)]) {
                const view = new DataView(e.memory.buffer), data = view.getUint32(parts, true);
                const handles = texts.map((_, i) => view.getUint32(data + 4 * i, true));
                const result = e.join(parts, separator), again = e.join(parts, separator);
                assert.equal(abi.decode('string', result), expected);
                assert.equal(abi.decode('string', again), expected);
                if (texts.length === 1) assert.equal(result, handles[0], 'singleton returns its entry');
                if (texts.length > 1 && expected.length > 0) {
                    assert.notEqual(result, again, 'positive multi-entry results are fresh');
                    for (const input of [...handles, separator, parts]) assert.notEqual(result, input);
                    const current = new DataView(e.memory.buffer);
                    const outputData = current.getUint32(result, true), bytes = current.getUint32(result + 4, true);
                    assert.deepEqual(Buffer.from(e.memory.buffer, outputData, bytes), Buffer.from(expected));
                    for (const h of [...handles, separator]) {
                        const inputData = current.getUint32(h, true), length = current.getUint32(h + 4, true);
                        assert.ok(outputData + bytes <= inputData || inputData + length <= outputData,
                            'copied result does not overlap any nonempty input payload');
                    }
                }
                assert.deepEqual(abi.decode('string[]', parts), texts);
                assert.equal(abi.decode('string', separator), separatorText);
                if (texts.length > 0) {
                    e.set_first(parts, abi.encode('string', 'replaced'));
                    assert.equal(abi.decode('string', result), expected, 'slot replacement cannot mutate a result');
                    for (let i = 0; i < handles.length; i++) assert.equal(abi.decode('string', handles[i]), texts[i]);
                }
            }
        }
        assert.equal(abi.decode('string', e.contextual()), '');
        assert.equal(abi.decode('string', e.literal()), 'éλ\0λ🙂');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_inspects_post_separator_fixed_writes_shrink_and_empty_to_nonempty_growth() {
    run(
        r#"
        fn fixed(parts: string[]) -> string {
            let alias = parts;
            return string_join({ effect(1); parts }, {
                effect(2); alias[0] = "λ"; parts = ["redirected"]; ":"
            });
        }
        fn shrink(parts: string[]) -> string {
            return string_join({ effect(3); parts }, {
                effect(4); let removed = parts.remove(0); let last = parts.pop();
                retain(removed); retain(last); ","
            });
        }
        fn clear(parts: string[]) -> string {
            return string_join(parts, { let first = parts.pop(); let second = parts.pop(); "unused" });
        }
        fn append(parts: string[]) -> string {
            return string_join(parts, { effect(5); parts.reserve(1024); parts.push("é"); parts.push("🙂"); "λ" });
        }
        fn movable(parts: string[]) -> string[] { return resizable(parts); }
        "#,
        r#"
        const events = [], retained = [];
        effect = n => events.push(n);
        retain = h => retained.push([h, abi.decode('string', h)]);
        const fixed = abi.encode('string[]', ['old', 'tail']);
        assert.equal(abi.decode('string', e.fixed(fixed)), 'λ:tail');
        assert.deepEqual(abi.decode('string[]', fixed), ['λ', 'tail']);
        const parts = e.movable(abi.encode('string[]', ['first', 'middle', 'last']));
        const data = new DataView(e.memory.buffer).getUint32(parts, true);
        const middle = new DataView(e.memory.buffer).getUint32(data + 4, true);
        assert.equal(e.shrink(parts), middle, 'post-separator singleton reuses the surviving entry');
        assert.deepEqual(abi.decode('string[]', parts), ['middle']);
        const clearing = e.movable(abi.encode('string[]', ['a', 'b']));
        assert.equal(abi.decode('string', e.clear(clearing)), '');
        assert.deepEqual(abi.decode('string[]', clearing), []);
        const empty = e.movable(abi.encode('string[]', []));
        assert.equal(abi.decode('string', e.append(empty)), 'éλ🙂');
        assert.deepEqual(abi.decode('string[]', empty), ['é', '🙂']);
        assert.deepEqual(events, [1, 2, 3, 4, 5]);
        assert.deepEqual(retained.map(([, text]) => text), ['first', 'last']);
        for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_argument_traps_stop_later_effects_and_completed_mutations_and_results_survive() {
    run(
        r#"
        fn fail() -> string { let empty: string[] = []; return empty[0]; }
        fn parts_trap() -> string {
            return string_join({ effect(1); let unused = fail(); ["a"] }, { effect(2); "," });
        }
        fn separator_trap(parts: string[]) -> string {
            return string_join({ effect(3); parts }, {
                effect(4); parts[0] = "changed";
                retain(string_join(parts, "λ")); fail()
            });
        }
        fn ordered(parts: string[]) -> string {
            return string_join({ effect(5); parts }, { effect(6); produce(7) });
        }
        "#,
        r#"
        const events = [], retained = [];
        effect = n => events.push(n);
        retain = h => retained.push([h, abi.decode('string', h)]);
        assert.throws(() => e.parts_trap(), WebAssembly.RuntimeError);
        assert.deepEqual(events, [1]);
        const parts = abi.encode('string[]', ['old', 'tail']);
        events.length = 0;
        assert.throws(() => e.separator_trap(parts), WebAssembly.RuntimeError);
        assert.deepEqual(events, [3, 4]);
        assert.deepEqual(abi.decode('string[]', parts), ['changed', 'tail']);
        assert.deepEqual(retained.map(([, text]) => text), ['changedλtail']);
        for (const text of ['', '🙂']) {
            events.length = 0;
            produce = n => { events.push(n); return abi.encode('string', text); };
            assert.equal(abi.decode('string', e.ordered(parts)), 'changed' + text + 'tail');
            assert.deepEqual(events, [5, 6, 7]);
        }
        events.length = 0;
        produce = n => { events.push(n); return 0; };
        assert.throws(() => e.ordered(parts), WebAssembly.RuntimeError);
        assert.deepEqual(events, [5, 6, 7]);
        for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_validates_unused_separator_outer_storage_and_every_entry_before_result_allocation() {
    run(
        r#"
        fn checked(parts: string[], separator: string) -> string {
            return string_join({ effect(1); parts }, { effect(2); separator });
        }
        "#,
        r#"
        const events = [];
        for (const texts of [[], [''], ['one'], ['', ''], ['a', '', 'b']]) {
            const parts = abi.encode('string[]', texts), separator = abi.encode('string', ',');
            // Post-entry corruption exercises builtin validation, not parameter checks.
            for (const target of [parts, separator]) {
                for (const corruptAt of [1, 2]) {
                    for (const [field, bad] of [[0, 0], [0, 0xfffffff8], [4, 0xffffffff]]) {
                        const address = target + field;
                        const original = new DataView(e.memory.buffer).getUint32(address, true);
                        events.length = 0;
                        effect = n => {
                            events.push(n);
                            if (n === corruptAt) new DataView(e.memory.buffer).setUint32(address, bad, true);
                        };
                        const before = e.__alloc(8) >>> 0, buffer = e.memory.buffer;
                        assert.throws(() => e.checked(parts, separator), WebAssembly.RuntimeError);
                        assert.deepEqual(events, [1, 2], 'both arguments precede join-specific checks');
                        assert.equal(e.memory.buffer, buffer);
                        assert.equal(e.__alloc(8) >>> 0, before + 8, 'invalid inputs precede result allocation');
                        new DataView(e.memory.buffer).setUint32(address, original, true);
                    }
                }
            }
            const data = new DataView(e.memory.buffer).getUint32(parts, true);
            for (let i = 0; i < texts.length; i++) {
                const address = data + i * 4;
                const entry = new DataView(e.memory.buffer).getUint32(address, true);
                for (const bad of [0, 1, 0xfffffff8]) {
                    events.length = 0;
                    effect = n => { events.push(n); if (n === 2) new DataView(e.memory.buffer).setUint32(address, bad, true); };
                    const before = e.__alloc(8) >>> 0;
                    assert.throws(() => e.checked(parts, separator), WebAssembly.RuntimeError);
                    assert.deepEqual(events, [1, 2]);
                    assert.equal(e.__alloc(8) >>> 0, before + 8, 'even the last invalid entry precedes allocation');
                    new DataView(e.memory.buffer).setUint32(address, entry, true);
                }
                const original = new DataView(e.memory.buffer).getUint32(entry, true);
                effect = n => { if (n === 2) new DataView(e.memory.buffer).setUint32(entry, 0, true); };
                assert.throws(() => e.checked(parts, separator), WebAssembly.RuntimeError);
                new DataView(e.memory.buffer).setUint32(entry, original, true);
            }
            effect = n => events.push(n);
            events.length = 0;
            assert.throws(() => e.checked(0, separator), WebAssembly.RuntimeError);
            assert.throws(() => e.checked(parts, 0), WebAssembly.RuntimeError);
            assert.deepEqual(events, [], 'entry validation remains before argument effects');
            assert.equal(abi.decode('string', e.checked(parts, separator)), texts.join(','));
            assert.deepEqual(abi.decode('string[]', parts), texts);
            assert.equal(abi.decode('string', separator), ',');
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_accepts_raw_nonowning_shared_storage_and_memory_end_empty_payloads_without_utf8_scans() {
    run(
        r#"
        fn join(parts: string[], separator: string) -> string { return string_join(parts, separator); }
        "#,
        r#"
        const raw = (data, length) => {
            const h = e.__alloc(8) >>> 0, view = new DataView(e.memory.buffer);
            view.setUint32(h, data, true); view.setUint32(h + 4, length, true); return h;
        };
        const payload = e.__alloc(8) >>> 0;
        new Uint8Array(e.memory.buffer, payload, 4).set([0xc3, 0xa9, 0, 0x61]);
        const first = raw(payload, 2), second = raw(payload + 2, 2);
        const slots = e.__alloc(16) >>> 0;
        let view = new DataView(e.memory.buffer);
        view.setUint32(slots + 4, first, true); view.setUint32(slots + 8, second, true);
        const parts = raw(slots + 4, 2), separator = abi.encode('string', 'λ');
        const result = e.join(parts, separator);
        assert.equal(abi.decode('string', result), 'éλ\0a');
        assert.deepEqual(abi.decode('string[]', parts), ['é', '\0a']);
        assert.deepEqual([...new Uint8Array(e.memory.buffer, payload, 4)], [0xc3, 0xa9, 0, 0x61]);
        const empty = raw(e.memory.buffer.byteLength, 0);
        const emptyArray = raw(e.memory.buffer.byteLength, 0);
        assert.equal(abi.decode('string', e.join(emptyArray, empty)), '');
        view = new DataView(e.memory.buffer); view.setUint32(slots + 4, empty, true);
        assert.equal(e.join(raw(slots + 4, 1), separator), empty);
        // Deliberately violate trusted-host UTF-8 validity: structural validation must not add a scan.
        new Uint8Array(e.memory.buffer, payload, 2).set([0xff, 0x80]);
        view = new DataView(e.memory.buffer); view.setUint32(slots + 4, first, true);
        const copied = e.join(parts, empty), current = new DataView(e.memory.buffer);
        assert.equal(current.getUint32(copied + 4, true), 4);
        assert.deepEqual([...new Uint8Array(e.memory.buffer, current.getUint32(copied, true), 4)],
            [0xff, 0x80, 0, 0x61]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_preserves_nested_capture_forwarding_live_stack_prefixes_and_recursive_state() {
    run(
        r#"
        fn sum(a: number, b: number, c: number) -> number { return a + b + c; }
        fn nested(parts: string[]) -> number {
            let alias = parts;
            let separator = ":";
            let outer = () => {
                let inner = () => string_join(parts, {
                    effect(1); alias[0] = "λ"; separator = "🙂";
                    string_join([separator, separator], "")
                });
                inner()
            };
            return 100 + sum(7, string_byte_len(outer()),
                3 + string_byte_len(string_join(["a", "b"], string_join(["", ""], "λ"))));
        }
        fn arrays(parts: string[]) -> string[] {
            return ["prefix", string_join(parts, ":"), string_join([string_join(parts, ""), "!"], "λ")];
        }
        fn recursive(depth: number, parts: string[]) -> string {
            if depth == 0 { return string_join(parts, ":"); }
            return string_join([string_join(parts, "λ"), recursive(depth - 1, parts)], "🙂");
        }
        "#,
        r#"
        const events = [], parts = abi.encode('string[]', ['a', 'b']);
        effect = n => events.push(n);
        assert.equal(e.nested(parts), 125);
        assert.deepEqual(events, [1]);
        assert.deepEqual(abi.decode('string[]', parts), ['λ', 'b']);
        assert.deepEqual(abi.decode('string[]', e.arrays(parts)), ['prefix', 'λ:b', 'λbλ!']);
        assert.equal(abi.decode('string', e.recursive(3, parts)), 'λλb🙂λλb🙂λλb🙂λ:b');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_results_and_captured_movable_parts_survive_growth_reentry_and_caught_traps() {
    run(
        r#"
        fn movable(parts: string[]) -> string[] { return resizable(parts); }
        fn mutate(parts: string[]) { parts.reserve(1024); parts[0] = "λ"; parts.push("🙂"); }
        fn publish(parts: string[]) -> string {
            let result = string_join(parts, "é"); retain(result); return result;
        }
        fn fail(parts: string[]) {
            retain(string_join(parts, "λ"));
            let empty: string[] = []; let invalid = empty[0];
        }
        fn outer(parts: string[]) -> string {
            return string_concat("outer:", string_join({ effect(1); parts }, { effect(2); ":" }));
        }
        "#,
        r#"
        const retained = [], events = [], source = abi.encode('string[]', ['a', 'b']);
        const parts = e.movable(source), published = [];
        retain = h => retained.push([h, abi.decode('string', h)]);
        published.push(e.publish(parts));
        const oldData = new DataView(e.memory.buffer).getUint32(parts, true);
        effect = n => {
            events.push(n);
            if (n !== 2) return;
            e.mutate(parts);
            assert.notEqual(new DataView(e.memory.buffer).getUint32(parts, true), oldData,
                'separator reentry actually relocates the captured payload');
            const large = abi.encode('string[]', ['x'.repeat(90000), '🙂'.repeat(90000)]);
            const marker = e.__alloc(8) >>> 0, remaining = e.memory.buffer.byteLength - (marker + 8);
            if (remaining > 0) e.__alloc(remaining);
            const before = e.memory.buffer, grown = e.publish(large);
            assert.notEqual(e.memory.buffer, before, 'join allocation itself grows memory');
            assert.equal(abi.decode('string', grown), 'x'.repeat(90000) + 'é' + '🙂'.repeat(90000));
            assert.throws(() => e.fail(parts), WebAssembly.RuntimeError);
            published.push(e.publish(parts));
            for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        };
        const outer = e.outer(parts);
        assert.deepEqual(events, [1, 2]);
        assert.equal(abi.decode('string', outer), 'outer:λ:b:🙂');
        assert.deepEqual(abi.decode('string[]', parts), ['λ', 'b', '🙂']);
        assert.deepEqual(abi.decode('string[]', source), ['a', 'b']);
        assert.equal(abi.decode('string', published[0]), 'aéb');
        assert.equal(abi.decode('string', published[1]), 'λébé🙂');
        effect = () => {};
        assert.throws(() => e.fail(parts), WebAssembly.RuntimeError);
        e.publish(abi.encode('string[]', ['界'.repeat(150000), 'tail']));
        for (const [h, text] of retained) assert.equal(abi.decode('string', h), text);
        assert.equal(abi.decode('string', outer), 'outer:λ:b:🙂');
        "#,
    );
}

#[test]
#[ignore = "requires Node.js 24+"]
fn join_checked_fragment_and_separator_expansion_traps_before_allocation_without_large_memory() {
    run(
        r#"
        fn join(parts: string[], separator: string) -> string { return string_join(parts, separator); }
        "#,
        r#"
        const makeRepeated = (entry, count) => {
            const data = e.__alloc(count * 4) >>> 0, h = e.__alloc(8) >>> 0;
            new Uint32Array(e.memory.buffer, data, count).fill(entry);
            const view = new DataView(e.memory.buffer);
            view.setUint32(h, data, true); view.setUint32(h + 4, count, true); return h;
        };
        const empty = abi.encode('string', ''), fragment = abi.encode('string', 'a'.repeat(65536));
        const small = abi.encode('string[]', ['a', 'b']), separator = abi.encode('string', 'λ');
        const retained = e.join(small, separator);
        for (const [entry, count, sep, expectedSize] of [
            [fragment, 65536, empty, 0x100000000n],
            [empty, 65537, fragment, 0x100000000n],
            // Payload fits u32, but descriptor-inclusive memory32 sizing cannot.
            [abi.encode('string', 'b'.repeat(65537)), 65535, empty, 0xffffffffn]
        ]) {
            const view = new DataView(e.memory.buffer);
            assert.equal(BigInt(count) * BigInt(view.getUint32(entry + 4, true)) +
                BigInt(count - 1) * BigInt(view.getUint32(sep + 4, true)), expectedSize);
            const parts = makeRepeated(entry, count), before = e.__alloc(8) >>> 0, buffer = e.memory.buffer;
            assert.throws(() => e.join(parts, sep), WebAssembly.RuntimeError);
            assert.equal(e.memory.buffer, buffer, 'overflow must not attempt memory growth');
            assert.equal(e.__alloc(8) >>> 0, before + 8, 'overflow must not advance the heap cursor');
            assert.equal(abi.decode('string', retained), 'aλb');
            assert.equal(abi.decode('string', e.join(small, separator)), 'aλb', 'later joins still succeed');
        }
        "#,
    );
}
