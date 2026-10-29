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
            Command::new("node")
                .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
                .arg("-e")
                .arg(script),
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
fn node_copy_encoding_stays_fixed_and_decoding_snapshots_current_resizable_contents() {
    run(
        r#"
        fn copy_numbers(a: number[]) -> number[] { return resizable(a); }
        fn copy_flags(a: bool[]) -> bool[] { return resizable(a); }
        fn copy_texts(a: string[]) -> string[] { return resizable(a); }
        fn reserve_numbers(a: number[]) { a.reserve({ effect(1); 0 }); }
        fn reserve_flags(a: bool[]) { a.reserve({ effect(1); 0 }); }
        fn reserve_texts(a: string[]) { a.reserve({ effect(1); 0 }); }
        fn push_numbers(a: number[]) { a.push({ effect(2); 9 }); }
        fn push_flags(a: bool[]) { a.push({ effect(2); true }); }
        fn push_texts(a: string[]) { a.push({ effect(2); "new" }); }
        fn pop_numbers(a: number[]) -> number { return a.pop(); }
        fn pop_flags(a: bool[]) -> bool { return a.pop(); }
        fn pop_texts(a: string[]) -> string { return a.pop(); }
        fn remove_numbers(a: number[]) -> number { return a.remove({ effect(3); -0 }); }
        fn remove_flags(a: bool[]) -> bool { return a.remove({ effect(3); -0 }); }
        fn remove_texts(a: string[]) -> string { return a.remove({ effect(3); -0 }); }
        "#,
        r#"
        const assert = require('node:assert/strict');
        const {instantiate} = require('./runtime/index.cjs');
        const {createAbi, readDescriptor} = require('./runtime/abi.cjs');
        const calls = [];
        const {instance, call} = instantiate(require('node:fs').readFileSync(0),
            {env:{effect(n) {calls.push(n);}}});
        const e = instance.exports, abi = createAbi(() => instance);
        for (const [kind,type,width,input,changed,added] of [
            ['numbers','number[]',8,[1,2,3],77,9],
            ['flags','bool[]',1,[true,false,true],false,true],
            ['texts','string[]',4,['é','λ','last'],'changed','new']]) {
            const expected = input.slice();
            const fixed = abi.encode(type,input);
            const before = readDescriptor(e.memory,fixed,width);
            input[0] = changed;
            assert.deepEqual(abi.decode(type,fixed),expected);
            for (const op of ['reserve','push','pop','remove']) {
                assert.throws(() => call(op+'_'+kind,[fixed]), error =>
                    error.stage === 'invocation' && error.cause instanceof WebAssembly.RuntimeError);
                assert.deepEqual(calls,[],'fixed authority must precede argument effects');
                const after = readDescriptor(e.memory,fixed,width);
                assert.equal(after.data,before.data);
                assert.equal(after.length,before.length);
                assert.deepEqual(abi.decode(type,fixed),expected);
            }
            // Movability comes from guest construction, never from host registration.
            const movable = call('copy_'+kind,[fixed]);
            assert.notEqual(movable,fixed);
            const snapshot = abi.decode(type,movable);
            snapshot[0] = changed;
            assert.deepEqual(abi.decode(type,movable),expected);
            call('reserve_'+kind,[movable]);
            call('push_'+kind,[movable]);
            assert.deepEqual(abi.decode(type,movable),[...expected,added]);
            const scalar = type.slice(0,-2);
            assert.equal(abi.decode(scalar,call('pop_'+kind,[movable])),added);
            assert.equal(abi.decode(scalar,call('remove_'+kind,[movable])),expected[0]);
            assert.deepEqual(abi.decode(type,movable),expected.slice(1));
            assert.deepEqual(abi.decode(type,fixed),expected);
            assert.deepEqual(snapshot,[changed,...expected.slice(1)]);
            assert.deepEqual(calls,[1,2,3]);
            calls.length = 0;
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn raw_hosts_reacquire_movable_payloads_across_reentry_traps_and_actual_memory_growth() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([10,20,30]); }
        fn flags() -> bool[] { return resizable([true,false,true]); }
        fn texts() -> string[] { return resizable(["first","middle","last"]); }
        fn reserve_numbers(a: number[], n: number) { a.reserve(n); }
        fn reserve_flags(a: bool[], n: number) { a.reserve(n); }
        fn reserve_texts(a: string[], n: number) { a.reserve(n); }
        fn push_numbers(a: number[]) { a.push(40); }
        fn push_flags(a: bool[]) { a.push(false); }
        fn push_texts(a: string[]) { a.push("added"); }
        fn trap_numbers(a: number[]) { a.remove(0); a.remove(len(a)); }
        fn trap_flags(a: bool[]) { a.remove(0); a.remove(len(a)); }
        fn trap_texts(a: string[]) { a.remove(0); a.remove(len(a)); }
        fn reenter_numbers(a: number[]) { a[0] = { effect(1); 99 }; }
        fn reenter_flags(a: bool[]) { a[0] = { effect(2); true }; }
        fn reenter_texts(a: string[]) { a[0] = { effect(3); "replacement" }; }
        fn pop_numbers(a: number[]) -> number { return a.pop(); }
        fn pop_flags(a: bool[]) -> bool { return a.pop(); }
        fn pop_texts(a: string[]) -> string { return a.pop(); }
        "#,
        r#"
        const assert = require('node:assert/strict');
        const {instantiate} = require('./runtime/index.cjs');
        const {createAbi,readDescriptor} = require('./runtime/abi.cjs');
        let active, callbacks = [];
        const {instance,call} = instantiate(require('node:fs').readFileSync(0),{env:{effect(tag) {
            const {kind,h,width,type,initial} = active;
            callbacks.push(tag);
            const before = readDescriptor(e.memory,h,width);
            call('reserve_'+kind,[h,2000000]);
            const after = readDescriptor(e.memory,h,width);
            assert.notEqual(after.buffer,before.buffer,'reserve must grow memory in this fixture');
            assert.notEqual(after.data,before.data);
            assert.equal(after.length,3);
            call('push_'+kind,[h]);
            assert.throws(() => e['trap_'+kind](h),WebAssembly.RuntimeError);
            assert.deepEqual(abi.decode(type,h),[...initial.slice(1),active.added]);
        }}});
        const e = instance.exports, abi = createAbi(() => instance);
        const retained = [];
        for (const [kind,type,width,initial,replacement,added,tag] of [
            ['numbers','number[]',8,[10,20,30],99,40,1],
            ['flags','bool[]',1,[true,false,true],true,false,2],
            ['texts','string[]',4,['first','middle','last'],'replacement','added',3]]) {
            const fixed = abi.encode(type,initial);
            const fixedBefore = readDescriptor(e.memory,fixed,width);
            const h = call(kind);
            const original = readDescriptor(e.memory,h,width);
            // A small relocation in the same buffer proves pointer refresh is independent of view refresh.
            call('reserve_'+kind,[h,128]);
            const small = readDescriptor(e.memory,h,width);
            assert.equal(small.buffer,original.buffer);
            assert.notEqual(small.data,original.data);
            assert.deepEqual(abi.decode(type,h),initial);
            active = {kind,type,width,h,initial,added};
            call('reenter_'+kind,[h]);
            assert.equal(callbacks.at(-1),tag);
            assert.deepEqual(abi.decode(type,h),[replacement,initial[2],added]);
            const postTrap = readDescriptor(e.memory,h,width);
            assert.equal(abi.decode(type.slice(0,-2),call('pop_'+kind,[h])),added);
            assert.deepEqual(abi.decode(type,h),[replacement,initial[2]]);
            assert.equal(readDescriptor(e.memory,h,width).data,postTrap.data);
            const fixedAfter = readDescriptor(e.memory,fixed,width);
            assert.equal(fixedAfter.data,fixedBefore.data,'fixed payload offsets survive growth');
            assert.notEqual(fixedAfter.buffer,fixedBefore.buffer,'views must still be refreshed');
            assert.deepEqual(abi.decode(type,fixed),initial);
            retained.push({kind,type,h,expected:[replacement,initial[2]]});
            for (const saved of retained) assert.deepEqual(abi.decode(saved.type,saved.h),saved.expected);
        }
        assert.deepEqual(callbacks,[1,2,3]);
        for (const saved of retained) {
            call('reserve_'+saved.kind,[saved.h,4000000]);
            assert.deepEqual(abi.decode(saved.type,saved.h),saved.expected);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_all_types_preserve_identity_capacity_and_payload() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([11, 22, 33, 44]); }
        fn flags() -> bool[] { return resizable([true, false, true, false]); }
        fn texts() -> string[] { return resizable(["α🙂", "", "third", "last"]); }
        fn pop_numbers(a: number[]) -> number { let alias = a; return alias.pop(); }
        fn pop_flags(a: bool[]) -> bool { let alias = a; return alias.pop(); }
        fn pop_texts(a: string[]) -> string { let alias = a; return alias.pop(); }
        fn remove_numbers(a: number[], i: number) -> number { return a.remove(i); }
        fn remove_flags(a: bool[], i: number) -> bool { return a.remove(i); }
        fn remove_texts(a: string[], i: number) -> string { return a.remove(i); }
        fn push_numbers(a: number[]) { a.push(55); }
        fn push_flags(a: bool[]) { a.push(true); }
        fn push_texts(a: string[]) { a.push("again"); }
        "#,
        r#"
        const assert=require('node:assert/strict');
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:()=>assert.fail('unexpected callback')}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        for(const [kind,width,expected,added] of [
            ['numbers',8,[11,22,33,44],55],['flags',1,[1,0,1,0],1],
            ['texts',4,['α🙂','','third','last'],'again']]) {
            const h=e[kind](),p=word(h),capacity=v().getBigUint64(h+24,true),identity=v().getBigUint64(h+40,true);
            const scalar=x=>kind==='texts'?text(x):x;
            const values=()=>Array.from({length:word(h+4)},(_,i)=>scalar(kind==='numbers'?
                v().getFloat64(p+width*i,true):kind==='flags'?v().getUint8(p+i):word(p+width*i)));
            assert.equal(scalar(e['remove_'+kind](h,1)),expected[1]);
            assert.deepEqual(values(),[expected[0],expected[2],expected[3]]);
            assert.equal(scalar(e['pop_'+kind](h)),expected[3]);
            assert.equal(scalar(e['remove_'+kind](h,-0)),expected[0]);
            assert.equal(scalar(e['pop_'+kind](h)),expected[2]);
            assert.deepEqual(values(),[]);
            for(const op of [()=>e['pop_'+kind](h),()=>e['remove_'+kind](h,0)])
                assert.throws(op,WebAssembly.RuntimeError);
            assert.equal(word(h),p);assert.equal(word(h+4),0);
            assert.equal(v().getBigUint64(h+24,true),capacity);
            assert.equal(v().getBigUint64(h+40,true),identity);
            for(let i=0;i<4;i++)e['push_'+kind](h);
            assert.equal(word(h),p);assert.deepEqual(values(),Array(4).fill(added));
            assert.equal(v().getBigUint64(h+24,true),capacity);
            assert.equal(v().getBigUint64(h+40,true),identity);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn remove_checks_authority_before_once_only_argument_and_numeric_domain_after_effects() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7, 19]); }
        fn flags() -> bool[] { return resizable([true, false]); }
        fn texts() -> string[] { return resizable(["old", "new"]); }
        fn fixed_numbers() -> number[] { return [7, 19]; }
        fn fixed_flags() -> bool[] { return [true, false]; }
        fn fixed_texts() -> string[] { return ["old", "new"]; }
        fn remove_numbers(a: number[], i: number) -> number { return a.remove({ effect(1); i }); }
        fn remove_flags(a: bool[], i: number) -> bool { return a.remove({ effect(2); i }); }
        fn remove_texts(a: string[], i: number) -> string { return a.remove({ effect(3); i }); }
        fn pop_numbers(a: number[]) -> number { return a.pop(); }
        fn pop_flags(a: bool[]) -> bool { return a.pop(); }
        fn pop_texts(a: string[]) -> string { return a.pop(); }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let mutate=()=>{};
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);mutate();}}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true),put=(p,n)=>v().setUint32(p,n,true);
        for(const [kind,event,width] of [['numbers',1,8],['flags',2,1],['texts',3,4]]) {
            const h=e[kind](),other=e[kind](),p=word(h),fixed=e['fixed_'+kind]();
            const forged=e.__alloc(8);put(forged,p);put(forged+4,2);
            const snapshot=()=>Array.from(new Uint8Array(e.memory.buffer,h,48)).concat(
                Array.from(new Uint8Array(e.memory.buffer,p,width*2)));
            for(const bad of [fixed,forged,0,4]) {
                events.length=0;
                assert.throws(()=>e['remove_'+kind](bad,0),WebAssembly.RuntimeError);
                assert.throws(()=>e['pop_'+kind](bad),WebAssembly.RuntimeError);
                assert.deepEqual(events,[]);
            }
            for(const index of [-1,-Number.MIN_VALUE,0.5,NaN,Infinity,-Infinity,2,4294967296]) {
                events.length=0;const before=snapshot();
                assert.throws(()=>e['remove_'+kind](h,index),WebAssembly.RuntimeError);
                assert.deepEqual(events,[event]);assert.deepEqual(snapshot(),before);
            }
            for(const [offset,bad] of [[0,word(other)],[4,0]]) {
                put(h+offset,bad);events.length=0;const before=snapshot();
                assert.throws(()=>e['remove_'+kind](h,0),WebAssembly.RuntimeError);
                assert.throws(()=>e['pop_'+kind](h),WebAssembly.RuntimeError);
                assert.deepEqual(events,[]);assert.deepEqual(snapshot(),before);
                put(h,p);put(h+4,2);
                mutate=()=>put(h+offset,bad);events.length=0;
                assert.throws(()=>e['remove_'+kind](h,0),WebAssembly.RuntimeError);
                assert.deepEqual(events,[event]);assert.equal(word(h+offset),bad);
                mutate=()=>{};put(h,p);put(h+4,2);
            }
            events.length=0;e['remove_'+kind](h,-0);assert.deepEqual(events,[event]);assert.equal(word(h+4),1);
        }
        const n=e.numbers();events.length=0;
        assert.throws(()=>e.remove_flags(n,0),WebAssembly.RuntimeError);
        assert.throws(()=>e.pop_flags(n),WebAssembly.RuntimeError);assert.deepEqual(events,[]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn remove_uses_captured_receiver_and_post_argument_length_after_nested_effects() {
    run(
        r#"
        fn seed() -> number[] { return resizable([7, 19, 31]); }
        fn append(a: number[]) { a.push(43); }
        fn pop(a: number[]) -> number { return a.pop(); }
        fn remove(a: number[], i: number) -> number { return a.remove(i); }
        fn nested(a: number[], replacement: number[], i: number) -> number {
            let receiver = a;
            let alias = receiver;
            let index = () => {
                effect(1);
                receiver = replacement;
                effect(2);
                return i;
            };
            let invoke = () => receiver.remove(index());
            let result = invoke();
            effect(3);
            return result;
        }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let a,b,mode;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                events.push(n);
                if(n===1){assert.equal(word(a+4),3);assert.equal(e.remove(a,0),7);}
                if(n===2){
                    assert.equal(word(a+4),2);assert.equal(word(b+4),3);
                    if(mode==='grow')e.append(a);else assert.equal(e.pop(a),31);
                    e.__alloc(70000);
                }
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const values=h=>Array.from({length:word(h+4)},(_,i)=>v().getFloat64(word(h)+8*i,true));
        mode='grow';a=e.seed();b=e.seed();
        assert.equal(e.nested(a,b,2),43);assert.deepEqual(events,[1,2,3]);
        assert.deepEqual(values(a),[19,31]);assert.deepEqual(values(b),[7,19,31]);
        mode='shrink';a=e.seed();b=e.seed();events.length=0;
        assert.throws(()=>e.nested(a,b,1),WebAssembly.RuntimeError);
        assert.deepEqual(events,[1,2]);assert.deepEqual(values(a),[19]);assert.deepEqual(values(b),[7,19,31]);
        assert.equal(e.pop(a),19);assert.equal(word(a+4),0);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_validate_only_returned_bool_or_string_before_any_mutation() {
    run(
        r#"
        fn flags() -> bool[] { return resizable([true, false, true, false]); }
        fn texts() -> string[] { return resizable(["first", "second", "third", "last"]); }
        fn pop_flags(a: bool[]) -> bool { return a.pop(); }
        fn pop_texts(a: string[]) -> string { return a.pop(); }
        fn remove_flags(a: bool[], i: number) -> bool { return a.remove({ effect(1); i }); }
        fn remove_texts(a: string[], i: number) -> string { return a.remove({ effect(2); i }); }
        fn push_flags(a: bool[]) { a.push(true); }
        fn push_texts(a: string[]) { a.push("after"); }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        for(const [kind,width,event] of [['flags',1,1],['texts',4,2]]) {
            const get=p=>width===1?v().getUint8(p):word(p);
            const put=(p,x)=>width===1?v().setUint8(p,x):v().setUint32(p,x,true);
            for(const op of ['pop','remove']) {
                const h=e[kind](),p=word(h),index=op==='pop'?3:1,slot=p+width*index,good=get(slot);
                for(const bad of kind==='flags'?[2,255]:[0,4,e.memory.buffer.byteLength-4]) {
                    put(slot,bad);events.length=0;
                    const before=Array.from(new Uint8Array(e.memory.buffer,h,48)).concat(
                        Array.from(new Uint8Array(e.memory.buffer,p,width*4)));
                    assert.throws(()=>e[op+'_'+kind](h,index),WebAssembly.RuntimeError);
                    assert.deepEqual(events,op==='remove'?[event]:[]);
                    assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer,h,48)).concat(
                        Array.from(new Uint8Array(e.memory.buffer,p,width*4))),before);
                }
                put(slot,good);assert.equal(e[op+'_'+kind](h,index),good);assert.equal(word(h+4),3);
                e['push_'+kind](h);assert.equal(word(h+4),4);assert.equal(word(h),p);
            }
            // Both the untouched prefix and the shifted tail can contain invalid unreturned slots.
            const h=e[kind](),p=word(h),good=get(p+width);
            put(p,kind==='flags'?255:0);put(p+width*2,kind==='flags'?2:4);
            assert.equal(e['remove_'+kind](h,1),good);assert.equal(word(h+4),3);
            assert.equal(get(p),kind==='flags'?255:0);assert.equal(get(p+width),kind==='flags'?2:4);
            const last=get(p+width*2);assert.equal(e['pop_'+kind](h),last);assert.equal(word(h+4),2);
            assert.equal(word(h),p);assert.equal(get(p+width),kind==='flags'?2:4);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_numeric_values_and_raw_shift_preserve_special_value_bits() {
    run(
        r#"
        fn seed() -> number[] { return resizable([0, 0, 0, 0, 0, 0, 0, 0]); }
        fn pop(a: number[]) -> number { return a.pop(); }
        fn remove(a: number[], i: number) -> number { return a.remove(i); }
        "#,
        r#"
        const assert=require('node:assert/strict');
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:()=>assert.fail('unexpected callback')}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const bits=[0x8000000000000000n,0n,0x7ff0000000000000n,0xfff0000000000000n,
            1n,0x8000000000000001n,0x7ff8000000000042n,0x7fefffffffffffffn];
        for(const mode of ['pop','remove']) {
            const h=e.seed(),p=word(h),remaining=bits.slice();
            bits.forEach((b,i)=>v().setBigUint64(p+8*i,b,true));
            while(remaining.length) {
                const i=mode==='pop'?remaining.length-1:Math.floor(remaining.length/2);
                const expected=remaining.splice(i,1)[0];
                const scratch=new DataView(new ArrayBuffer(8));scratch.setBigUint64(0,expected,true);
                const result=mode==='pop'?e.pop(h):e.remove(h,i);
                assert(Object.is(result,scratch.getFloat64(0,true)));
                assert.equal(word(h),p);assert.equal(word(h+4),remaining.length);
                remaining.forEach((b,j)=>assert.equal(v().getBigUint64(p+8*j,true),b));
            }
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_pending_numeric_stores_and_all_compounds_use_saved_old_value_and_current_index() {
    for (operator, expected) in [
        ("=", "4"),
        ("+=", "21"),
        ("-=", "13"),
        ("*=", "68"),
        ("/=", "4.25"),
        ("%=", "1"),
    ] {
        let source = r#"
            fn seed() -> number[] { return resizable([7, 17, 31]); }
            fn write(a: number[], b: number[], shrink: bool) -> number[] {
                let receiver = a;
                let alias = receiver;
                let change = () => {
                    if shrink { let removed = alias.pop(); let again = alias.pop(); }
                    else { let removed = alias.remove(0); }
                    receiver = b;
                    effect(20);
                };
                receiver[{ effect(10); 1 }] OP { change(); 4 };
                return receiver;
            }
        "#
        .replace("OP", operator);
        let script = r#"
            const assert=require('node:assert/strict'),events=[];let a;
            const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
                {env:{effect:n=>{events.push(n);if(n===20)v().setFloat64(word(a)+8,999,true);}}}).exports;
            const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
            const values=h=>Array.from({length:word(h+4)},(_,i)=>v().getFloat64(word(h)+8*i,true));
            a=e.seed();const b=e.seed(),p=word(a);
            assert.equal(e.write(a,b,0),b);assert.deepEqual(events,[10,20]);
            assert.deepEqual(values(a),[17,EXPECTED]);assert.equal(word(a),p);
            assert.deepEqual(values(b),[7,17,31]);
            a=e.seed();const shrunk=word(a);events.length=0;
            assert.throws(()=>e.write(a,b,1),WebAssembly.RuntimeError);
            assert.deepEqual(events,[10,20]);assert.deepEqual(values(a),[7]);assert.equal(word(a),shrunk);
            assert.equal(v().getFloat64(shrunk+8,true),999,'post-RHS trap must not store even into spare capacity');
            assert.deepEqual(values(b),[7,17,31]);
        "#
        .replace("EXPECTED", expected);
        run(&source, &script);
    }
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_pending_bool_and_string_stores_shift_current_index_or_trap_after_shrink() {
    run(
        r#"
        fn flags() -> bool[] { return resizable([true, false, true]); }
        fn texts() -> string[] { return resizable(["first", "second", "third"]); }
        fn write_flags(a: bool[], shrink: bool) {
            a[{ effect(1); 1 }] = {
                if shrink { let x = a.pop(); let y = a.pop(); }
                else { let x = a.remove(0); }
                effect(2); false
            };
        }
        fn write_texts(a: string[], shrink: bool) {
            a[{ effect(3); 1 }] = {
                if shrink { let x = a.pop(); let y = a.pop(); }
                else { let x = a.remove(0); }
                effect(4); "replacement"
            };
        }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        for(const [kind,width,markers] of [['flags',1,[1,2]],['texts',4,[3,4]]]) {
            const read=p=>kind==='flags'?v().getUint8(p):text(word(p));
            let h=e[kind](),p=word(h);events.length=0;
            e['write_'+kind](h,0);assert.deepEqual(events,markers);assert.equal(word(h),p);assert.equal(word(h+4),2);
            assert.equal(read(p),kind==='flags'?0:'second');assert.equal(read(p+width),kind==='flags'?0:'replacement');
            h=e[kind]();p=word(h);const old=read(p+width);events.length=0;
            assert.throws(()=>e['write_'+kind](h,1),WebAssembly.RuntimeError);
            assert.deepEqual(events,markers);assert.equal(word(h),p);assert.equal(word(h+4),1);
            assert.equal(read(p),kind==='flags'?1:'first');assert.equal(read(p+width),old);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_recursive_captured_and_reentrant_pending_writes_survive_caught_inner_traps() {
    run(
        r#"
        fn seed(n: number) -> number[] { return resizable([n, n + 1, n + 2, n + 3]); }
        fn recursive(a: number[], depth: number) -> number {
            let alias = a;
            let take = () => alias.pop();
            if depth > 0 { return recursive(a, depth - 1) + take(); }
            return take();
        }
        fn remove(a: number[], i: number) -> number { return a.remove(i); }
        fn write(a: number[], marker: number) {
            let receiver = a;
            let rhs = () => { effect(marker); return 4; };
            receiver[{ effect(marker + 10); 1 }] += rhs();
        }
        fn fail(a: number[]) { a[3] = { let x = a.pop(); effect(3); 99 }; }
        fn append(a: number[]) { a.push(77); }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let outer,inner,failed,depth=0;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                events.push(n);
                if(n===1){
                    assert.equal(depth++,0);e.write(inner,2);
                    assert.throws(()=>e.fail(failed),WebAssembly.RuntimeError);
                    assert.equal(e.remove(outer,0),17);e.__alloc(70000);depth--;
                }else if(n===2){assert.equal(e.remove(inner,0),31);}
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const values=h=>Array.from({length:word(h+4)},(_,i)=>v().getFloat64(word(h)+8*i,true));
        const recursive=e.seed(1);assert.equal(e.recursive(recursive,2),9);assert.deepEqual(values(recursive),[1]);
        outer=e.seed(17);inner=e.seed(31);failed=e.seed(43);
        e.write(outer,1);assert.deepEqual(events,[11,1,12,2,3]);
        assert.deepEqual(values(outer),[18,22,20]);assert.deepEqual(values(inner),[32,36,34]);
        assert.deepEqual(values(failed),[43,44,45]);
        e.append(failed);e.write(failed,4);assert.deepEqual(values(failed),[43,48,45,77]);
        assert.deepEqual(events,[11,1,12,2,3,14,4]);assert.equal(depth,0);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn pop_remove_iteration_keeps_initial_count_checks_later_yields_and_observes_shifts() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([1, 2, 3]); }
        fn flags() -> bool[] { return resizable([true, false, true]); }
        fn texts() -> string[] { return resizable(["first", "second", "third"]); }
        fn numbers_loop(a: number[], shift: bool) {
            let first = true;
            for x in a {
                effect(x);
                if first { first = false; if shift { let y = a.remove(0); } else { let y = a.pop(); } }
            }
        }
        fn flags_loop(a: bool[], shift: bool) {
            let first = true;
            for x in a {
                if x { effect(1); } else { effect(0); }
                if first { first = false; if shift { let y = a.remove(0); } else { let y = a.pop(); } }
            }
        }
        fn texts_loop(a: string[], shift: bool) {
            let first = true;
            for x in a {
                if x == "first" { effect(1); } else if x == "second" { effect(2); } else { effect(3); }
                if first { first = false; if shift { let y = a.remove(0); } else { let y = a.pop(); } }
            }
        }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const word=p=>new DataView(e.memory.buffer).getUint32(p,true);
        for(const kind of ['numbers','flags','texts'])for(const shift of [0,1]) {
            const h=e[kind](),p=word(h);events.length=0;
            assert.throws(()=>e[kind+'_loop'](h,shift),WebAssembly.RuntimeError);
            assert.deepEqual(events,kind==='flags'?(shift?[1,1]:[1,0]):(shift?[1,3]:[1,2]));
            assert.equal(word(h),p);assert.equal(word(h+4),2);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn remove_validates_post_argument_consumed_value_without_rolling_back_argument_effects() {
    run(
        r#"
        fn flags() -> bool[] { return resizable([true, false, true]); }
        fn texts() -> string[] { return resizable(["first", "second", "third"]); }
        fn remove_flags(a: bool[]) -> bool { return a.remove({ effect(1); 1 }); }
        fn remove_texts(a: string[]) -> string { return a.remove({ effect(2); 1 }); }
        fn pop_flags(a: bool[]) -> bool { return a.pop(); }
        fn pop_texts(a: string[]) -> string { return a.pop(); }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let mutate=()=>{},after;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);mutate();}}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        for(const [kind,width,event] of [['flags',1,1],['texts',4,2]]) {
            const h=e[kind](),p=word(h);
            const snapshot=()=>Array.from(new Uint8Array(e.memory.buffer,h,48)).concat(
                Array.from(new Uint8Array(e.memory.buffer,p,width*3)));
            const slot=p+width,good=width===1?v().getUint8(slot):word(slot);
            // The invalid string handle is aligned and in bounds, but its descriptor is invalid.
            const invalid=e.__alloc(8);v().setUint32(invalid,0,true);v().setUint32(invalid+4,1,true);
            mutate=()=>{
                if(width===1)v().setUint8(slot,255);else v().setUint32(slot,invalid,true);
                after=snapshot();
            };
            events.length=0;
            assert.throws(()=>e['remove_'+kind](h),WebAssembly.RuntimeError);
            assert.deepEqual(events,[event]);assert.deepEqual(snapshot(),after);
            mutate=()=>{};
            if(width===1)v().setUint8(slot,good);else v().setUint32(slot,good,true);
            assert.equal(e['remove_'+kind](h),good);assert.equal(word(h+4),2);
            // A nested successful shrink survives the outer operation's now-out-of-bounds index.
            const fresh=e[kind](),freshp=word(fresh);events.length=0;
            mutate=()=>{e['pop_'+kind](fresh);e['pop_'+kind](fresh);};
            assert.throws(()=>e['remove_'+kind](fresh),WebAssembly.RuntimeError);
            assert.deepEqual(events,[event]);assert.equal(word(fresh),freshp);assert.equal(word(fresh+4),1);
            mutate=()=>{};e['pop_'+kind](fresh);assert.equal(word(fresh+4),0);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_traces_once_only_item_and_post_item_length_through_alias() {
    run(
        r#"
        fn seed() -> number[] { return resizable([7]); }
        fn append(a: number[]) {
            let alias = a;
            a.push({ effect(1); alias.push(19); effect(2); 31 });
            effect(3);
        }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let a;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);assert.equal(word(a+4),n===1?1:n===2?2:3);}}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        a=e.seed();e.append(a);
        assert.deepEqual(events,[1,2,3]);assert.equal(word(a+4),3);
        assert.deepEqual([0,1,2].map(i=>v().getFloat64(word(a)+8*i,true)),[7,19,31]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_captured_receiver_survives_rebinding_nested_push_and_host_reentry() {
    run(
        r#"
        fn seed(n: number) -> number[] { return resizable([n]); }
        fn append(a: number[], n: number) { a.push(n); }
        fn nested(original: number[], replacement: number[]) -> number[] {
            let receiver = original;
            let alias = receiver;
            let item = () => {
                alias.push(19);
                receiver = replacement;
                effect(1);
                return 31;
            };
            let invoke = () => { receiver.push(item()); };
            invoke();
            return receiver;
        }
        "#,
        r#"
        const assert=require('node:assert/strict');let original,replacement,calls=0;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                assert.equal(n,1);calls++;assert.equal(word(original+4),2);
                e.append(original,23);e.append(replacement,41);e.__alloc(70000);
                assert.equal(word(original+4),3);
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const values=h=>Array.from({length:word(h+4)},(_,i)=>v().getFloat64(word(h)+8*i,true));
        original=e.seed(7);replacement=e.seed(81);const old=word(original);
        assert.equal(e.nested(original,replacement),replacement);assert.equal(calls,1);
        assert.notEqual(word(original),old);
        assert.deepEqual(values(original),[7,19,23,31]);assert.deepEqual(values(replacement),[81,41]);
        e.append(original,43);assert.deepEqual(values(original),[7,19,23,31,43]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_all_types_spare_growth_and_numeric_special_values() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([]); }
        fn flags() -> bool[] { return resizable([]); }
        fn texts() -> string[] { return resizable([]); }
        fn append_number(a: number[], n: number) { a.push({ effect(1); n }); }
        fn flag(a: bool[], n: bool) { a.push({ effect(2); n }); }
        fn text(a: string[], n: string) { a.push({ effect(3); n }); }
        fn literal() -> string { return "α🙂"; }
        fn empty() -> string { return ""; }
        fn reserve(a: number[]) { a.reserve(100); }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        const nums=[-0,0,NaN,Infinity,-Infinity,Number.MIN_VALUE,-Number.MIN_VALUE,Number.MAX_VALUE,1.25];
        const n=e.numbers(),np=word(n);
        nums.forEach((x,i)=>{e.append_number(n,x);assert.equal(word(n+4),i+1);});
        assert.notEqual(word(n),np);
        nums.forEach((x,i)=>assert(Object.is(v().getFloat64(word(n)+8*i,true),x)));
        e.reserve(n);const spare=word(n);e.append_number(n,42);assert.equal(word(n),spare);
        assert.equal(v().getFloat64(word(n)+8*nums.length,true),42);
        const f=e.flags(),fp=word(f);const flags=Array.from({length:17},(_,i)=>i%2);
        flags.forEach((x,i)=>{e.flag(f,x);assert.equal(word(f+4),i+1);if(i<8)assert.equal(word(f),fp);});
        assert.notEqual(word(f),fp);assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer,word(f),17)),flags);
        const s=e.texts(),sp=word(s),handles=[e.literal(),e.empty(),e.literal(),e.empty(),e.literal()];
        handles.forEach((h,i)=>{e.text(s,h);assert.equal(word(s+4),i+1);if(i<2)assert.equal(word(s),sp);});
        assert.notEqual(word(s),sp);
        assert.deepEqual(handles.map((_,i)=>text(word(word(s)+4*i))),['α🙂','','α🙂','','α🙂']);
        assert.deepEqual(events,[...Array(10).fill(1),...Array(17).fill(2),...Array(5).fill(3)]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_pending_stores_and_all_compounds_follow_current_payload_after_reuse() {
    for (operator, expected) in [
        ("=", "4"),
        ("+=", "21"),
        ("-=", "13"),
        ("*=", "68"),
        ("/=", "4.25"),
        ("%=", "1"),
    ] {
        let source = r#"
            fn seed(n: number) -> number[] { return resizable([n]); }
            fn reuse() -> number[] { return resizable([7]); }
            fn write(a: number[], b: number[], index: number) -> number[] {
                let receiver = a;
                let grow = () => { receiver.push(23); receiver = b; effect(20); };
                receiver[{ effect(10); index }] OP { grow(); 4 };
                return receiver;
            }
        "#
        .replace("OP", operator);
        let script = r#"
            const assert=require('node:assert/strict');let a,old,reused;const events=[];
            const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
                {env:{effect:n=>{
                    events.push(n);
                    if(n===20){
                        assert.notEqual(word(a),old);assert.equal(word(a+4),2);
                        assert.equal(v().getFloat64(word(a)+8,true),23);
                        reused=e.reuse();assert.equal(word(reused),old,'exercise actual displaced-buffer reuse');
                        v().setFloat64(word(a),999,true);
                    }
                }}}).exports;
            const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
            a=e.seed(17);const b=e.seed(81);old=word(a);
            assert.equal(e.write(a,b,-0),b);assert.deepEqual(events,[10,20]);
            assert.equal(v().getFloat64(word(a),true),EXPECTED);assert.equal(word(a+4),2);
            assert.equal(v().getFloat64(word(a)+8,true),23);
            assert.equal(v().getFloat64(word(b),true),81);assert.equal(word(b+4),1);
            assert.equal(v().getFloat64(word(reused),true),7);
            for(const index of [-1,2,0.5,NaN,Infinity]) {
                events.length=0;assert.throws(()=>e.write(a,b,index),WebAssembly.RuntimeError);
                assert.deepEqual(events,[10]);assert.equal(word(a+4),2);
            }
        "#
        .replace("EXPECTED", expected);
        run(&source, &script);
    }
}

#[test]
#[ignore = "requires Node.js"]
fn push_iteration_uses_initial_length_and_current_elements_for_all_types() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([1, 2]); }
        fn flags() -> bool[] { return resizable([true, false]); }
        fn texts() -> string[] { return resizable(["old", "later"]); }
        fn numbers_loop(a: number[]) -> number {
            let sum = 0;
            for x in a { sum += x; a.push(99); a[1] = 7; effect(1); }
            return sum;
        }
        fn flags_loop(a: bool[]) -> number {
            let count = 0;
            for x in a { if x { count += 1; } a.push(false); a[1] = true; effect(2); }
            return count;
        }
        fn texts_loop(a: string[]) -> bool {
            let seen = false;
            for x in a { if x == "changed" { seen = true; } a.push("new"); a[1] = "changed"; effect(3); }
            return seen;
        }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const word=p=>new DataView(e.memory.buffer).getUint32(p,true);
        for(const [kind,result] of [['numbers',8],['flags',2],['texts',1]]) {
            const h=e[kind]();assert.equal(e[kind+'_loop'](h),result);assert.equal(word(h+4),4);
        }
        assert.deepEqual(events,[1,1,2,2,3,3]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_validates_authority_before_effects_and_rechecks_post_item_state() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7]); }
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["old"]); }
        fn fixed_numbers() -> number[] { return [7]; }
        fn fixed_flags() -> bool[] { return [true]; }
        fn fixed_texts() -> string[] { return ["old"]; }
        fn push_numbers(a: number[]) { a.push({ effect(1); 19 }); }
        fn push_flags(a: bool[]) { a.push({ effect(2); false }); }
        fn push_texts(a: string[]) { a.push({ effect(3); "new" }); }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let mutate=()=>{};
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);mutate();}}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true),put=(p,n)=>v().setUint32(p,n,true);
        for(const [kind,event,width] of [['numbers',1,8],['flags',2,1],['texts',3,4]]) {
            const h=e[kind](),other=e[kind](),p=word(h),fixed=e['fixed_'+kind]();
            const forged=e.__alloc(8);put(forged,p);put(forged+4,1);
            for(const bad of [fixed,forged,0,4]) {
                events.length=0;assert.throws(()=>e['push_'+kind](bad),WebAssembly.RuntimeError);
                assert.deepEqual(events,[]);
            }
            for(const [offset,bad] of [[0,word(other)],[4,0]]) {
                events.length=0;put(h+offset,bad);
                assert.throws(()=>e['push_'+kind](h),WebAssembly.RuntimeError);assert.deepEqual(events,[]);
                put(h,p);put(h+4,1);
                const before=Array.from(new Uint8Array(e.memory.buffer,p,width));
                mutate=()=>put(h+offset,bad);
                assert.throws(()=>e['push_'+kind](h),WebAssembly.RuntimeError);
                assert.deepEqual(events,[event]);assert.equal(word(h+offset),bad);
                assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer,p,width)),before);
                mutate=()=>{};put(h,p);put(h+4,1);
            }
            e['push_'+kind](h);assert.equal(word(h+4),2);
        }
        const n=e.numbers();events.length=0;
        assert.throws(()=>e.push_flags(n),WebAssembly.RuntimeError);assert.deepEqual(events,[]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_copies_old_bool_and_string_slots_lazily_but_consumes_new_items() {
    run(
        r#"
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["old"]); }
        fn push_flag(a: bool[]) { a.push({ effect(1); false }); }
        fn push_text(a: string[]) { a.push({ effect(2); "new" }); }
        fn flag(a: bool[], i: number) -> bool { return a[i]; }
        fn text(a: string[], i: number) -> string { return a[i]; }
        fn consume_flag(a: bool[], source: bool[]) { a.push({ effect(3); source[0] }); }
        fn consume_text(a: string[], item: string) { a.push({ effect(4); item }); }
        fn literal() -> string { return "item"; }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let mutate=()=>{};
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);mutate(n);}}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true),put=(p,n)=>v().setUint32(p,n,true);
        const f=e.flags(),s=e.texts(),fp=word(f),sp=word(s);
        v().setUint8(fp,255);put(sp,0);
        for(let i=0;i<9;i++){e.push_flag(f);e.push_text(s);}
        assert.notEqual(word(f),fp);assert.notEqual(word(s),sp);
        assert.equal(word(f+4),10);assert.equal(word(s+4),10);
        assert.equal(v().getUint8(word(f)),255);assert.equal(word(word(s)),0);
        assert.throws(()=>e.flag(f,0),WebAssembly.RuntimeError);assert.throws(()=>e.text(s,0),WebAssembly.RuntimeError);
        assert.equal(e.flag(f,9),0);assert(e.text(s,9)>0);
        const before=events.length,fl=word(f+4),fdata=word(f);
        assert.throws(()=>e.consume_flag(f,f),WebAssembly.RuntimeError);
        assert.deepEqual(events.slice(before),[3]);assert.equal(word(f+4),fl);assert.equal(word(f),fdata);
        // The string was valid at function entry and is invalidated only by item effects.
        const item=e.literal(),saved=word(item),sl=word(s+4),sdata=word(s);
        mutate=n=>{if(n===4)put(item,0);};
        assert.throws(()=>e.consume_text(s,item),WebAssembly.RuntimeError);
        assert.equal(events.at(-1),4);assert.equal(word(s+4),sl);assert.equal(word(s),sdata);
        mutate=()=>{};put(item,saved);e.consume_text(s,item);assert.equal(word(s+4),sl+1);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_pending_bool_and_string_stores_survive_growth_and_reused_payloads() {
    run(
        r#"
        fn flags() -> bool[] { return resizable([true, true, true, true, true, true, true, true]); }
        fn texts() -> string[] { return resizable(["old", "kept"]); }
        fn reuse() -> number[] { return resizable([7]); }
        fn write_flags(a: bool[], b: bool[]) -> bool[] {
            let receiver = a;
            let grow = () => { receiver.push(true); receiver = b; effect(1); };
            receiver[0] = { grow(); false };
            return receiver;
        }
        fn write_texts(a: string[], b: string[]) -> string[] {
            let receiver = a;
            let grow = () => { receiver.push("appended"); receiver = b; effect(2); };
            receiver[0] = { grow(); "changed" };
            return receiver;
        }
        "#,
        r#"
        const assert=require('node:assert/strict'),compiled=new WebAssembly.Module(require('node:fs').readFileSync(0));
        for(const kind of ['flags','texts']) {
            let a,old,reused,calls=0;
            const e=new WebAssembly.Instance(compiled,{env:{effect:n=>{
                assert.equal(n,kind==='flags'?1:2);calls++;
                assert.notEqual(word(a),old);reused=e.reuse();assert.equal(word(reused),old);
            }}}).exports;
            const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
            const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
            a=e[kind]();const b=e[kind]();old=word(a);
            assert.equal(e['write_'+kind](a,b),b);assert.equal(calls,1);
            assert.equal(word(a+4),kind==='flags'?9:3);
            if(kind==='flags'){assert.equal(v().getUint8(word(a)),0);assert.equal(v().getUint8(word(b)),1);}
            else{assert.equal(text(word(word(a))),'changed');assert.equal(text(word(word(b))),'old');assert.equal(text(word(word(a)+8)),'appended');}
            assert.equal(v().getFloat64(word(reused),true),7);
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_reentrant_pending_writes_and_trapped_rhs_keep_invocation_state_separate() {
    run(
        r#"
        fn seed(n: number) -> number[] { return resizable([n]); }
        fn append(a: number[], n: number) { a.push(n); }
        fn write(a: number[], marker: number) { a[0] += { effect(marker); 4 }; }
        fn fail(a: number[]) { a[0] = { a.push({ a.push(53); effect(3); a[99] }); 0 }; }
        "#,
        r#"
        const assert=require('node:assert/strict'),events=[];let outer,inner,failed;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                events.push(n);
                if(n===1){
                    e.write(inner,2);assert.throws(()=>e.fail(failed),WebAssembly.RuntimeError);
                    e.append(outer,23);e.__alloc(70000);v().setFloat64(word(outer),999,true);
                }else if(n===2){e.append(inner,37);v().setFloat64(word(inner),999,true);}
                else if(n===3){v().setFloat64(word(failed),47,true);}
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const values=h=>Array.from({length:word(h+4)},(_,i)=>v().getFloat64(word(h)+8*i,true));
        outer=e.seed(17);inner=e.seed(31);failed=e.seed(43);
        const old=[outer,inner,failed].map(word);
        e.write(outer,1);assert.deepEqual(events,[1,2,3]);
        [outer,inner,failed].forEach((h,i)=>assert.notEqual(word(h),old[i]));
        assert.deepEqual(values(outer),[21,23]);assert.deepEqual(values(inner),[35,37]);
        assert.deepEqual(values(failed),[47,53]);
        e.append(failed,59);e.write(failed,4);assert.deepEqual(values(failed),[51,53,59]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn push_bounded_allocation_failure_preserves_post_item_state_for_all_types() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7]); }
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["old"]); }
        fn append_number(a: number[]) { a.push(19); }
        fn flag(a: bool[]) { a.push(false); }
        fn text(a: string[]) { a.push("inner"); }
        fn outer_numbers(a: number[]) { a.push({ a[0] = 23; effect(1); 31 }); }
        fn outer_flags(a: bool[]) { a.push({ a[0] = false; effect(2); true }); }
        fn outer_texts(a: string[]) { a.push({ a[0] = "changed"; effect(3); "outer" }); }
        fn reserve_numbers(a: number[]) { a.reserve(0); }
        fn reserve_flags(a: bool[]) { a.reserve(0); }
        fn reserve_texts(a: string[]) { a.reserve(0); }
        "#,
        r#"
        const assert=require('node:assert/strict');
        // Bound the instance, not the requested guest work: no large allocation is attempted.
        const bytes=new Uint8Array(require('node:fs').readFileSync(0));let cursor=8,patched=false;
        const leb=()=>{let n=0,shift=0,b;do{b=bytes[cursor++];n+=(b&127)*2**shift;shift+=7;}while(b&128);return n;};
        while(cursor<bytes.length){
            const id=bytes[cursor++],size=leb(),end=cursor+size;
            if(id===5){
                assert.equal(leb(),1);assert.equal(leb(),1);
                const initial=leb(),start=cursor;leb();const width=cursor-start;let n=initial;
                for(let i=0;i<width;i++){bytes[start+i]=(n&127)|(i+1<width?128:0);n=Math.floor(n/128);}
                assert.equal(n,0);patched=true;break;
            }
            cursor=end;
        }
        assert(patched);assert(WebAssembly.validate(bytes));const compiled=new WebAssembly.Module(bytes);
        for(const [kind,append,event,width] of [['numbers','append_number',1,8],['flags','flag',2,1],['texts','text',3,4]]){
            let active,snapshot,prepared,completed=0,calls=0;
            const e=new WebAssembly.Instance(compiled,{env:{effect:n=>{
                assert.equal(n,event);calls++;let trapped=false;
                // Fill through reentry until a bounded replacement allocation fails.
                for(let i=0;i<100000;i++){
                    try{e[append](active);completed++;}
                    catch(error){assert(error instanceof WebAssembly.RuntimeError);trapped=true;break;}
                }
                assert(trapped,'bounded memory must force a growth failure');assert(completed>0);
                prepared=[word(active),word(active+4)];
                snapshot=Array.from(new Uint8Array(e.memory.buffer,prepared[0],prepared[1]*width));
            }}}).exports;
            const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
            const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
            active=e[kind]();const initialBytes=e.memory.buffer.byteLength;
            assert.throws(()=>e['outer_'+kind](active),WebAssembly.RuntimeError);assert.equal(calls,1);
            assert.deepEqual([word(active),word(active+4)],prepared);assert.equal(word(active+4),completed+1);
            assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer,word(active),word(active+4)*width)),snapshot);
            assert.equal(e.memory.buffer.byteLength,initialBytes);
            if(kind==='numbers')assert.equal(v().getFloat64(word(active),true),23);
            if(kind==='flags')assert.equal(v().getUint8(word(active)),0);
            if(kind==='texts')assert.equal(text(word(word(active))),'changed');
            e['reserve_'+kind](active);assert.deepEqual([word(active),word(active+4)],prepared);
            assert(!Object.keys(e).some(name=>name.startsWith('__m3_')));
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn reserve_preserves_aliases_length_and_values_for_all_types() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7, -0, 0 / 0]); }
        fn flags() -> bool[] { return resizable([true, false, true]); }
        fn texts() -> string[] { return resizable(["α", "", "last"]); }
        fn reserve_numbers(a: number[], n: number) { let alias = a; alias.reserve({ effect(1); n }); }
        fn reserve_flags(a: bool[], n: number) { let alias = a; alias.reserve({ effect(2); n }); }
        fn reserve_texts(a: string[], n: number) { let alias = a; alias.reserve({ effect(3); n }); }
        "#,
        r#"
        const assert=require('node:assert/strict'), events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
        const string=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        for(const [kind,width] of [['numbers',8],['flags',1],['texts',4]]) {
            const h=e[kind](), original=word(h);
            const bytes=()=>Array.from(new Uint8Array(e.memory.buffer,word(h),3*width));
            const expected=bytes();
            e['reserve_'+kind](h,0); assert.equal(word(h),original);
            e['reserve_'+kind](h,-0); assert.equal(word(h),original);
            e['reserve_'+kind](h,100); assert.notEqual(word(h),original);
            assert.equal(word(h+4),3); assert.deepEqual(bytes(),expected);
            const grown=word(h); e['reserve_'+kind](h,1); assert.equal(word(h),grown);
            assert.equal(word(h+4),3); assert.deepEqual(bytes(),expected);
            if(kind==='numbers') {
                assert.equal(v().getFloat64(word(h),true),7);
                assert(Object.is(v().getFloat64(word(h)+8,true),-0));
                assert(Number.isNaN(v().getFloat64(word(h)+16,true)));
            } else if(kind==='texts') {
                assert.deepEqual([0,1,2].map(i=>string(word(word(h)+4*i))),['α','','last']);
            }
        }
        assert.deepEqual(events,[1,1,1,1,2,2,2,2,3,3,3,3]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn reserve_rejects_fixed_receivers_before_effects_and_checks_numeric_domain_after_effects() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7]); }
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["kept"]); }
        fn fixed_numbers() -> number[] { return [7]; }
        fn fixed_flags() -> bool[] { return [true]; }
        fn fixed_texts() -> string[] { return ["kept"]; }
        fn reserve_numbers(a: number[], n: number) { a.reserve({ effect(1); a[0] = 19; n }); }
        fn reserve_flags(a: bool[], n: number) { a.reserve({ effect(2); a[0] = false; n }); }
        fn reserve_texts(a: string[], n: number) { a.reserve({ effect(3); a[0] = "changed"; n }); }
        "#,
        r#"
        const assert=require('node:assert/strict'), events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
        const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        for(const [kind,width,event] of [['numbers',8,1],['flags',1,2],['texts',4,3]]) {
            const fixed=e['fixed_'+kind](), p=word(fixed);
            const before=Array.from(new Uint8Array(e.memory.buffer,p,width));
            assert.throws(()=>e['reserve_'+kind](fixed,0),WebAssembly.RuntimeError);
            assert.equal(events.length,0); assert.equal(word(fixed),p);
            assert.deepEqual(Array.from(new Uint8Array(e.memory.buffer,p,width)),before);
            const h=e[kind](), original=word(h);
            // Include element-count overflow, byte-domain overflow and a huge finite integer.
            for(const n of [-1,-Number.MIN_VALUE,0.5,NaN,Infinity,-Infinity,2**31,2**31/width,2**53,Number.MAX_VALUE]) {
                const count=events.length;
                assert.throws(()=>e['reserve_'+kind](h,n),WebAssembly.RuntimeError);
                assert.deepEqual(events.slice(count),[event]);
                assert.equal(word(h),original); assert.equal(word(h+4),1);
                if(kind==='numbers') assert.equal(v().getFloat64(word(h),true),19);
                if(kind==='flags') assert.equal(v().getUint8(word(h)),0);
                if(kind==='texts') assert.equal(text(word(word(h))),'changed');
            }
            for(const n of [0,-0]) {
                e['reserve_'+kind](h,n); assert.equal(word(h),original); assert.equal(word(h+4),1);
            }
            events.length=0;
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn reserve_checks_exact_runtime_authority_before_and_after_argument_effects() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7]); }
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["kept"]); }
        fn reserve_numbers(a: number[]) { a.reserve({ effect(1); 100 }); }
        fn reserve_flags(a: bool[]) { a.reserve({ effect(2); 100 }); }
        fn reserve_texts(a: string[]) { a.reserve({ effect(3); 100 }); }
        "#,
        r#"
        const assert=require('node:assert/strict'), events=[];let mutate=()=>{};
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);mutate();}}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true), put=(p,n)=>v().setUint32(p,n,true);
        for(const [kind,event] of [['numbers',1],['flags',2],['texts',3]]) {
            const h=e[kind](), p=word(h), other=e[kind]();
            const forged=e.__alloc(8);put(forged,p);put(forged+4,1);
            for(const bad of [0,4,forged]) {
                events.length=0;
                assert.throws(()=>e['reserve_'+kind](bad),WebAssembly.RuntimeError);
                assert.deepEqual(events,[]);
            }
            // A registered handle with a structurally valid but mismatching public descriptor
            // is not authorized. These mutations never touch private allocator metadata.
            for(const [offset,bad] of [[0,word(other)],[4,0],[0,0],[0,e.memory.buffer.byteLength-4]]) {
                put(h+offset,bad);events.length=0;
                assert.throws(()=>e['reserve_'+kind](h),WebAssembly.RuntimeError);
                assert.deepEqual(events,[]);put(h,p);put(h+4,1);
            }
            for(const [offset,bad] of [[0,word(other)],[4,0]]) {
                events.length=0; mutate=()=>put(h+offset,bad);
                assert.throws(()=>e['reserve_'+kind](h),WebAssembly.RuntimeError);
                assert.deepEqual(events,[event]);assert.equal(word(h+offset),bad);
                mutate=()=>{};put(h,p);put(h+4,1);
            }
            // A failed check neither retires nor poisons the original payload authority.
            e['reserve_'+kind](h); assert.notEqual(word(h),p);assert.equal(word(h+4),1);
        }
        const n=e.numbers();events.length=0;
        assert.throws(()=>e.reserve_flags(n),WebAssembly.RuntimeError);
        assert.deepEqual(events,[]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn reserve_copies_unconsumed_bool_and_string_slots_without_scanning() {
    run(
        r#"
        fn flags() -> bool[] { return resizable([true, false]); }
        fn texts() -> string[] { return resizable(["kept", "other"]); }
        fn reserve_flags(a: bool[], n: number) { a.reserve({ effect(1); n }); }
        fn reserve_texts(a: string[], n: number) { a.reserve({ effect(2); n }); }
        fn flag(a: bool[], i: number) -> bool { return a[i]; }
        fn text(a: string[], i: number) -> string { return a[i]; }
        "#,
        r#"
        const assert=require('node:assert/strict'), events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>events.push(n)}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
        const flags=e.flags(), texts=e.texts(), fp=word(flags), sp=word(texts);
        v().setUint8(fp+1,255); v().setUint32(sp+4,0,true);
        e.reserve_flags(flags,0);e.reserve_texts(texts,0);
        assert.equal(word(flags),fp);assert.equal(word(texts),sp);
        e.reserve_flags(flags,100);e.reserve_texts(texts,100);
        assert.notEqual(word(flags),fp);assert.notEqual(word(texts),sp);
        assert.equal(word(flags+4),2);assert.equal(word(texts+4),2);
        assert.equal(v().getUint8(word(flags)+1),255);assert.equal(word(word(texts)+4),0);
        assert.equal(e.flag(flags,0),1);assert(e.text(texts,0)>0);
        assert.throws(()=>e.flag(flags,1),WebAssembly.RuntimeError);
        assert.throws(()=>e.text(texts,1),WebAssembly.RuntimeError);
        assert.deepEqual(events,[1,2,1,2]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn reserve_captures_receiver_through_nested_reserve_rebinding_and_reentry() {
    run(
        r#"
        fn seed(n: number) -> number[] { return resizable([n]); }
        fn reserve(a: number[], n: number) { a.reserve(n); }
        fn nested(original: number[], replacement: number[]) -> number[] {
            let receiver = original;
            let argument = () => {
                receiver.reserve(100);
                receiver = replacement;
                effect(1);
                return 200;
            };
            receiver.reserve(argument());
            return receiver;
        }
        "#,
        r#"
        const assert=require('node:assert/strict');let original, replacement, afterOriginal, afterReplacement, calls=0;
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                assert.equal(n,1);calls++;e.reserve(original,300);e.reserve(replacement,100);
                afterOriginal=word(original);afterReplacement=word(replacement);
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
        original=e.seed(7);replacement=e.seed(19);
        const op=word(original), rp=word(replacement);
        assert.equal(e.nested(original,replacement),replacement);assert.equal(calls,1);
        assert.notEqual(word(original),op);assert.notEqual(word(replacement),rp);
        assert.equal(word(original),afterOriginal);assert.equal(word(replacement),afterReplacement);
        for(const [h,n] of [[original,7],[replacement,19]]) {
            assert.equal(word(h+4),1);assert.equal(v().getFloat64(word(h),true),n);
        }
        const p=word(original);e.reserve(original,200);assert.equal(word(original),p);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn ordinary_stores_follow_relocated_payload_not_rebound_receiver_or_reused_buffer() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7]); }
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["old"]); }
        fn reuse() -> number[] { return resizable([7]); }
        fn write_numbers(a: number[], b: number[], index: number) -> number[] {
            let receiver = a;
            let rebind = () => { receiver = b; };
            receiver[{ effect(10); index }] = { rebind(); a.reserve(100); effect(20); 42 };
            return receiver;
        }
        fn write_flags(a: bool[], b: bool[], index: number) -> bool[] {
            let receiver = a;
            let rebind = () => { receiver = b; };
            receiver[{ effect(10); index }] = { rebind(); a.reserve(100); effect(20); false };
            return receiver;
        }
        fn write_texts(a: string[], b: string[], index: number) -> string[] {
            let receiver = a;
            let rebind = () => { receiver = b; };
            receiver[{ effect(10); index }] = { rebind(); a.reserve(100); effect(20); "new" };
            return receiver;
        }
        "#,
        r#"
        const assert=require('node:assert/strict');
        const compiledModule=new WebAssembly.Module(require('node:fs').readFileSync(0));
        for(const kind of ['numbers','flags','texts']) {
            let a, displaced, reused; const events=[];
            const e=new WebAssembly.Instance(compiledModule,{env:{effect:n=>{
                events.push(n);
                if(n===20) {
                    assert.notEqual(word(a),displaced);
                    reused=e.reuse();assert.equal(word(reused),displaced,'must actually reuse the displaced payload');
                }
            }}}).exports;
            const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
            const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
            const read=h=>kind==='numbers'?v().getFloat64(word(h),true):kind==='flags'?v().getUint8(word(h)):text(word(word(h)));
            a=e[kind]();const b=e[kind](), before=read(b);displaced=word(a);
            assert.equal(e['write_'+kind](a,b,-0),b);
            assert.deepEqual(events,[10,20]);assert.equal(word(a+4),1);assert.equal(word(b+4),1);
            assert.equal(read(a),kind==='numbers'?42:kind==='flags'?0:'new');assert.equal(read(b),before);
            assert.equal(v().getFloat64(word(reused),true),7,'stale destination must not overwrite the reused buffer');
            for(const index of [-1,1,0.5,NaN,Infinity]) {
                events.length=0;const p=word(a);
                assert.throws(()=>e['write_'+kind](a,b,index),WebAssembly.RuntimeError);
                assert.deepEqual(events,[10]);assert.equal(word(a),p);
            }
        }
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn all_numeric_compounds_keep_pre_rhs_value_across_relocation_and_deterministic_reuse() {
    for (operator, expected) in [
        ("+=", "21"),
        ("-=", "13"),
        ("*=", "68"),
        ("/=", "4.25"),
        ("%=", "1"),
    ] {
        let source = r#"
            fn seed(n: number) -> number[] { return resizable([n]); }
            fn reuse() -> number[] { return resizable([7]); }
            fn write(a: number[], b: number[], index: number) -> number[] {
                let receiver = a;
                let rebind = () => { receiver = b; };
                receiver[{ effect(10); index }] OP { rebind(); a.reserve(100); effect(20); 4 };
                return receiver;
            }
        "#
        .replace("OP", operator);
        let script = r#"
            const assert=require('node:assert/strict');
            const compiledModule=new WebAssembly.Module(require('node:fs').readFileSync(0));
            let a, displaced, reused;const events=[];
            const e=new WebAssembly.Instance(compiledModule,{env:{effect:n=>{
                events.push(n);
                if(n===20) {
                    assert.notEqual(word(a),displaced);
                    reused=e.reuse();assert.equal(word(reused),displaced,'must actually reuse displaced storage');
                    // A compound retains the pre-RHS old value, not this intervening write.
                    v().setFloat64(word(a),999,true);
                }
            }}}).exports;
            const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
            a=e.seed(17);const b=e.seed(81);displaced=word(a);
            assert.equal(e.write(a,b,-0),b);assert.deepEqual(events,[10,20]);
            assert.equal(v().getFloat64(word(a),true),EXPECTED);
            assert.equal(v().getFloat64(word(b),true),81);
            assert.equal(v().getFloat64(word(reused),true),7);
            assert.equal(word(a+4),1);
            for(const index of [-1,1,0.5,NaN,Infinity]) {
                events.length=0;
                assert.throws(()=>e.write(a,b,index),WebAssembly.RuntimeError);
                assert.deepEqual(events,[10]);assert.equal(v().getFloat64(word(a),true),EXPECTED);
            }
        "#
        .replace("EXPECTED", expected);
        run(&source, &script);
    }
}

#[test]
#[ignore = "requires Node.js"]
fn reentrant_pending_writes_and_caught_inner_traps_use_distinct_invocation_scratch() {
    run(
        r#"
        fn seed(n: number) -> number[] { return resizable([n, n + 1]); }
        fn reserve(a: number[], n: number) { a.reserve(n); }
        fn write(a: number[], index: number, marker: number) {
            a[{ effect(marker + 10); index }] += { effect(marker); 4 };
        }
        fn fail(a: number[]) { a[0] = { a.reserve(100); effect(3); a[99] }; }
        "#,
        r#"
        const assert=require('node:assert/strict');let outer,inner,failed,depth=0;
        const events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                events.push(n);
                if(n===1) {
                    assert.equal(depth++,0);
                    e.write(inner,0,2);
                    assert.throws(()=>e.fail(failed),WebAssembly.RuntimeError);
                    e.reserve(outer,200); e.__alloc(70000);
                    v().setFloat64(word(outer)+8,1000,true);
                    depth--;
                } else if(n===2) {
                    e.reserve(inner,100);v().setFloat64(word(inner),2000,true);
                }
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer), word=p=>v().getUint32(p,true);
        const read=(h,i=0)=>v().getFloat64(word(h)+8*i,true);
        outer=e.seed(17);inner=e.seed(31);failed=e.seed(43);
        const op=word(outer),ip=word(inner),fp=word(failed);
        e.write(outer,1,1);assert.deepEqual(events,[11,1,12,2,3]);
        assert.notEqual(word(outer),op);assert.notEqual(word(inner),ip);assert.notEqual(word(failed),fp);
        assert.equal(read(outer),17);assert.equal(read(outer,1),22);
                assert.equal(read(inner),35);assert.equal(read(inner,1),32);assert.equal(read(failed),43);
        // A trapped invocation cannot leave a persistent pin or poison later writes/reserves.
        e.reserve(failed,200);e.write(failed,0,4);assert.equal(read(failed),47);
        assert.equal(read(outer),17);assert.equal(read(outer,1),22);assert.equal(read(inner),35);
        assert.deepEqual(events,[11,1,12,2,3,14,4]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn failed_rhs_retains_completed_reserve_and_reused_payload_without_final_store() {
    run(
        r#"
        fn seed() -> number[] { return resizable([17]); }
        fn reuse() -> number[] { return resizable([7]); }
        fn fail(a: number[]) { a[0] += { a.reserve(100); effect(1); a[99] }; }
        fn write(a: number[]) { a[0] += 4; }
        "#,
        r#"
        const assert=require('node:assert/strict');let a,old,reused;const events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{
                events.push(n);assert.notEqual(word(a),old);
                reused=e.reuse();assert.equal(word(reused),old);
                v().setFloat64(word(a),31,true);
            }}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        a=e.seed();old=word(a);
        assert.throws(()=>e.fail(a),WebAssembly.RuntimeError);assert.deepEqual(events,[1]);
        assert.notEqual(word(a),old);assert.equal(word(a+4),1);
        assert.equal(v().getFloat64(word(a),true),31);assert.equal(v().getFloat64(word(reused),true),7);
        e.write(a);assert.equal(v().getFloat64(word(a),true),35);
        assert.equal(v().getFloat64(word(reused),true),7);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn fixed_storage_pending_writes_remain_bound_to_the_original_place_in_m4_modules() {
    run(
        r#"
        fn movable() -> number[] { return resizable([7]); }
        fn fixed() -> number[] { return [17]; }
        fn store(a: number[], b: number[]) { let receiver = a; receiver[0] = { receiver = b; effect(1); 42 }; }
        fn compound(a: number[], b: number[]) { let receiver = a; receiver[0] += { receiver = b; effect(2); 4 }; }
        "#,
        r#"
        const assert=require('node:assert/strict');let a;const events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(require('node:fs').readFileSync(0)),
            {env:{effect:n=>{events.push(n);e.movable();e.__alloc(70000);v().setFloat64(word(a),999,true);}}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        a=e.fixed();const b=e.fixed(),p=word(a);
        e.store(a,b);assert.equal(word(a),p);assert.equal(v().getFloat64(p,true),42);
        e.compound(a,b);assert.equal(word(a),p);assert.equal(v().getFloat64(p,true),46);
        assert.equal(v().getFloat64(word(b),true),17);assert.deepEqual(events,[1,2]);
        "#,
    );
}

#[test]
#[ignore = "requires Node.js"]
fn bounded_allocation_failure_preserves_post_argument_published_arrays() {
    run(
        r#"
        fn numbers() -> number[] { return resizable([7]); }
        fn flags() -> bool[] { return resizable([true]); }
        fn texts() -> string[] { return resizable(["old"]); }
        fn reserve_numbers(a: number[], n: number) {
            a.reserve({ a.reserve(100); a[0] = 19; effect(1); n });
        }
        fn reserve_flags(a: bool[], n: number) {
            a.reserve({ a.reserve(100); a[0] = false; effect(2); n });
        }
        fn reserve_texts(a: string[], n: number) {
            a.reserve({ a.reserve(100); a[0] = "changed"; effect(3); n });
        }
        "#,
        r#"
        const assert=require('node:assert/strict');
        // Tighten only this instance's declared memory maximum to its initial size.
        // Preserve encoded widths/section sizes with legal zero-padded unsigned LEBs.
        // The failed request therefore cannot grow memory or provision a huge payload.
        const bytes=new Uint8Array(require('node:fs').readFileSync(0));let cursor=8,patched=false;
        const leb=()=>{let n=0,shift=0,b;do{b=bytes[cursor++];n+=(b&127)*2**shift;shift+=7;}while(b&128);return n;};
        while(cursor<bytes.length) {
            const id=bytes[cursor++],size=leb(),end=cursor+size;
            if(id===5) {
                assert.equal(leb(),1);assert.equal(leb(),1);
                const initial=leb(),start=cursor;leb();const width=cursor-start;
                let n=initial;
                for(let i=0;i<width;i++){bytes[start+i]=(n&127)|(i+1<width?128:0);n=Math.floor(n/128);}
                assert.equal(n,0);patched=true;break;
            }
            cursor=end;
        }
        assert(patched);assert(WebAssembly.validate(bytes));
        let active,prepared;const events=[];
        const e=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{effect:n=>{
            events.push(n);prepared=word(active);
        }}}).exports;
        const v=()=>new DataView(e.memory.buffer),word=p=>v().getUint32(p,true);
        const text=h=>Buffer.from(e.memory.buffer,word(h),word(h+4)).toString('utf8');
        const initialBytes=e.memory.buffer.byteLength;
        for(const [kind,event] of [['numbers',1],['flags',2],['texts',3]]) {
            active=e[kind]();const old=word(active);events.length=0;
            assert.throws(()=>e['reserve_'+kind](active,1000000),WebAssembly.RuntimeError);
            assert.deepEqual(events,[event]);assert.notEqual(prepared,old);
            assert.equal(word(active),prepared);assert.equal(word(active+4),1);
            assert.equal(e.memory.buffer.byteLength,initialBytes);
            if(kind==='numbers') assert.equal(v().getFloat64(word(active),true),19);
            if(kind==='flags') assert.equal(v().getUint8(word(active)),0);
            if(kind==='texts') assert.equal(text(word(word(active))),'changed');
            e['reserve_'+kind](active,0);assert.equal(word(active),prepared);
            assert.equal(word(active+4),1);assert.deepEqual(events,[event,event]);
        }
        assert(!Object.keys(e).some(name=>name.startsWith('__m3_')));
        "#,
    );
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
