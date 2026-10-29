#[path = "../../../tests/support/process.rs"]
mod test_process;

use std::{process::Command, time::Duration};
use was_compiler::{Compiler, OptimizationLevel, Type};

fn run(source: &str, script: &str) {
    let mut compiler = Compiler::new();
    compiler
        .register_host_function("effect", "env", "effect", vec![Type::Number], Type::Void)
        .unwrap();
    let mut enabled_bytes = None;
    for level in [
        OptimizationLevel::None,
        OptimizationLevel::Basic,
        OptimizationLevel::Standard,
        OptimizationLevel::Aggressive,
    ] {
        compiler.set_optimization_level(level);
        let bytes = compiler.compile_source(source).unwrap();
        assert_eq!(bytes, compiler.compile_source(source).unwrap());
        if level != OptimizationLevel::None {
            if let Some(expected) = &enabled_bytes {
                assert_eq!(&bytes, expected);
            } else {
                enabled_bytes = Some(bytes.clone());
            }
        }
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        let output = test_process::output(
            Command::new("node").arg("-e").arg(script),
            Some(&bytes),
            Duration::from_secs(30),
        );
        assert!(
            output.status.success(),
            "{level:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires Node.js"]
fn copies_all_types_once_with_distinct_handles_and_mutable_slots() {
    run(
        r#"
        fn numbers(a: number[]) -> number[] { return resizable({ effect(1); a }); }
        fn flags(a: bool[]) -> bool[] { return resizable({ effect(2); a }); }
        fn texts(a: string[]) -> string[] { return resizable({ effect(3); a }); }
        fn empty_numbers() -> number[] { return resizable([]); }
        fn empty_flags() -> bool[] { return resizable([]); }
        fn empty_texts() -> string[] { return resizable([]); }
        fn seed_numbers() -> number[] { return [0 / 0, -0, 1 / 0]; }
        fn seed_flags() -> bool[] { return [true, false, true]; }
        fn seed_texts() -> string[] { return ["α", "β", ""]; }
        fn write_number(a: number[]) { a[1] = 42; }
        fn write_flag(a: bool[]) { a[1] = true; }
        fn write_text(a: string[]) { a[1] = "changed"; }
        "#,
        r#"
        const assert = require('node:assert/strict'), events = [];
        const e = new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const view=()=>new DataView(e.memory.buffer), word=p=>view().getUint32(p,true);
        const number=(h,i)=>view().getFloat64(word(h)+8*i,true);
        const flag=(h,i)=>view().getUint8(word(h)+i);
        const string=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        const text=(h,i)=>string(word(word(h)+4*i));
        const all = new Set();
        for (const kind of ['numbers','flags','texts']) {
            const source=e['seed_'+kind](), copy=e[kind](source), again=e[kind](copy);
            for (const h of [source,copy,again]) { assert(!all.has(h)); all.add(h); assert.equal(word(h+4),3); }
            assert.notEqual(word(source),word(copy)); assert.notEqual(word(copy),word(again));
            if(kind==='numbers') {
                assert(Number.isNaN(number(copy,0))); assert(Object.is(number(copy,1),-0)); assert.equal(number(copy,2),Infinity);
                e.write_number(copy); assert(Object.is(number(source,1),-0)); assert(Object.is(number(again,1),-0));
                e.write_number(source); assert.equal(number(copy,1),42);
            } else if(kind==='flags') {
                assert.deepEqual([0,1,2].map(i=>flag(copy,i)),[1,0,1]);
                e.write_flag(copy); assert.equal(flag(source,1),0); assert.equal(flag(again,1),0);
            } else {
                assert.deepEqual([0,1,2].map(i=>text(copy,i)),['α','β','']);
                // Immutable string values may be shared, but their mutable array slots are not.
                assert.equal(word(word(source)),word(word(copy)));
                e.write_text(copy); assert.equal(text(source,1),'β'); assert.equal(text(again,1),'β');
            }
        }
        assert.deepEqual(events,[1,1,2,2,3,3]);
        for(const kind of ['numbers','flags','texts']) {
            const a=e['empty_'+kind](), b=e['empty_'+kind]();
            assert.notEqual(a,b); assert.notEqual(word(a),word(b)); assert.equal(word(a+4),0); assert.equal(word(b+4),0);
        }
        assert(!Object.keys(e).some(name=>name.startsWith('__m3_')));
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn construction_consumes_every_logical_element_and_preserves_source_after_traps() {
    run(
        r#"
        fn numbers(a: number[]) -> number[] { return resizable({ effect(1); a }); }
        fn flags(a: bool[]) -> bool[] { return resizable({ effect(2); a }); }
        fn texts(a: string[]) -> string[] { return resizable({ effect(3); a }); }
        fn flag_len(a: bool[]) -> number { return len(a); }
        fn text_len(a: string[]) -> number { return len(a); }
        "#,
        r#"
        const assert=require('node:assert/strict'), events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true), put=(p,x)=>v().setUint32(p,x,true);
        const descriptor=(p,n)=>{const h=e.__alloc(8);put(h,p);put(h+4,n);return h;};
        const fp=e.__alloc(8), flags=descriptor(fp,3);
        v().setUint8(fp,1);v().setUint8(fp+1,0);v().setUint8(fp+2,2);
        assert.equal(e.flag_len(flags),3);
        assert.throws(()=>e.flags(flags),WebAssembly.RuntimeError);
        assert.equal(word(flags),fp);assert.equal(word(flags+4),3);assert.equal(v().getUint8(fp+2),2);
        v().setUint8(fp+2,1);const good=e.flags(flags);assert.notEqual(good,flags);
        assert.deepEqual([0,1,2].map(i=>v().getUint8(word(good)+i)),[1,0,1]);
        const sp=e.__alloc(8), text=descriptor(sp,2), empty=descriptor(e.__alloc(8),0);
        put(sp,empty);put(sp+4,0);assert.equal(e.text_len(text),2);
        assert.throws(()=>e.texts(text),WebAssembly.RuntimeError);assert.equal(word(sp+4),0);
        put(sp+4,empty);const copied=e.texts(text);assert.equal(word(word(copied)+4),empty);
        const bad=descriptor(e.memory.buffer.byteLength-8,2);
        assert.throws(()=>e.numbers(bad),WebAssembly.RuntimeError);
        // Invalid outer storage is rejected at parameter entry, before body effects.
                assert.deepEqual(events,[2,2,3,3]);
        // An empty logical array does not consume spare storage.
        put(flags+4,0);v().setUint8(fp,255);assert.equal(word(e.flags(flags)+4),0);
        put(text+4,0);put(sp,0);assert.equal(word(e.texts(text)+4),0);
        assert.equal(word(good+4),3);assert.equal(v().getUint8(word(good)),1);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn captured_recursive_and_reentrant_construction_survives_growth_and_later_traps() {
    run(
        r#"
        fn build(depth: number) -> number[] {
            let original = [depth, 7];
            let copy = () => resizable(original);
            if depth > 0 { let inner = build(depth - 1); }
            effect(depth);
            return copy();
        }
        fn fail(a: number[]) { a[0] += 1; let unused = resizable(a); a[99] = 0; }
        fn read(a: number[]) -> number { return a[0]; }
        "#,
        r#"
        const assert=require('node:assert/strict'), events=[];let entered=false, retained=0;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);e.__alloc(70000);if(n===1&&!entered){entered=true;retained=e.build(0);}}}}).exports;
        const outer=e.build(2);assert.equal(e.read(outer),2);assert.equal(e.read(retained),0);
        assert.deepEqual(events,[0,1,0,2]);
        assert.throws(()=>e.fail(outer),WebAssembly.RuntimeError);assert.equal(e.read(outer),3);
        const later=e.build(0);assert.notEqual(later,retained);assert.equal(e.read(retained),0);assert.equal(e.read(outer),3);
        "#,
    );
}
