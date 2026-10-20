pub const CASES: &[(&str, &str, f64, Option<&str>)] = &[
    (
        "defaults_contracts_pipeline_match",
        r#"
fn stage(x: number, step: number = x + 2) -> number
    requires x > 0 requires step > x {
    let base = 10;
    let negative = -(base + 3);
    return match x {
        3 => if negative < -base { x + step - negative } else { 0 },
        _ => x + step,
    };
}
fn finish(value: number) -> number {
    let base = 7;
    let offset = -(base - 2);
    return match value {
        21 => if offset < -2 { value - offset } else { 0 },
        _ => 0,
    };
}
fn main() -> number {
    let seed = 1;
    let input = seed + 2;
    let first = input |> stage |> finish;
    return first + stage(step: 6, x: input);
}
fn reject() -> number { return 0 |> stage |> finish; }
"#,
        // stage(3, 5) = 21; finish(21) = 26; stage(3, 6) = 22.
        48.0,
        Some("reject"),
    ),
    (
        "loop_scopes_capture_exclusions",
        r#"
fn main() -> number {
    let captured = 10;
    let readonly = 7;
    let read = () => readonly;
    let observe = () => captured;
    let skipped = 9;
    false && { skipped = 0.5; true };
    let total = 0;
    for i in 0..3 {
        let readonly = 20;
        let offset = readonly - 3;
        let negative = -offset;
        let sample = () => {
            let base = 4;
            let negative = -(base + 2);
            return if negative < -base { captured + i - negative } else { 0 };
        };
        if i == 1 { continue; }
        if negative <= -17 && offset > 0 { total += sample(); }
    }
    captured += 0.5;
    let anchor = 3;
    let negative = -(anchor + 2);
    return if negative < -anchor { total + observe() + read() + skipped } else { 0 };
    skipped += 0.5;
}
"#,
        // Iterations 0 and 2 contribute 16 + 18; live capture is 10.5,
        // outer readonly is 7, and neither skipped write changes 9.
        60.5,
        None,
    ),
];

pub const DIAGNOSTICS: &[(&str, &str)] = &[
    (
        r#"
fn stage(x: number, step: number = x + 2) -> number requires x > 0 {
    return x + step;
}
fn main() -> number {
    let base = 10;
    let negative = -(base + 2);
    false && { match negative {
        -12 => (negative |> stage) < true,
        _ => false,
    } };
    return -negative;
}
"#,
        "Operator '<' cannot be applied to Number and Bool",
    ),
    (
        r#"
fn main() -> number {
    let captured = 10;
    let read = () => captured;
    for i in 0..1 {
        let base = 3;
        let negative = -(base + 2);
        if negative < -base {
            return read();
            -true;
        }
    }
    return 0;
}
"#,
        "Type mismatch: expected Number, found Bool",
    ),
];
