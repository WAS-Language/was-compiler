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
