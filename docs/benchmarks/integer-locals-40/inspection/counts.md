# Independent CLI WAT inspection

Only main (function index 1); parameters excluded from locals. Static counts, not runtime counts.
Binary hashes equal the corresponding timed modules. Other opcodes and helpers remain in full WAT.

| Case | Level | i32 locals | f64 locals | i64 locals | f64.convert_i32_s | i32 add/sub | f64 add/sub | f64.neg |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| locals_32 | None | 0 | 97 | 0 | 0 | 0 | 96 | 0 |
| locals_32 | Basic | 96 | 1 | 0 | 33 | 64 | 32 | 0 |
| compare_32 | None | 0 | 33 | 0 | 0 | 0 | 160 | 0 |
| compare_32 | Basic | 32 | 1 | 0 | 321 | 32 | 128 | 0 |
| negate_32 | None | 0 | 65 | 0 | 0 | 0 | 96 | 32 |
| negate_32 | Basic | 32 | 33 | 0 | 65 | 32 | 64 | 32 |
| loop_local | None | 2 | 4 | 3 | 0 | 0 | 2 | 0 |
| loop_local | Basic | 4 | 2 | 3 | 4 | 1 | 1 | 0 |
| mutable | None | 0 | 1 | 0 | 0 | 0 | 1 | 0 |
| mutable | Basic | 0 | 1 | 0 | 2 | 0 | 1 | 0 |
| capture | None | 2 | 1 | 0 | 0 | 0 | 0 | 0 |
| capture | Basic | 2 | 1 | 0 | 2 | 0 | 0 | 0 |
| negate_zero | None | 0 | 1 | 0 | 0 | 0 | 0 | 1 |
| negate_zero | Basic | 1 | 0 | 0 | 1 | 0 | 0 | 1 |
| negate_min | None | 0 | 1 | 0 | 0 | 0 | 0 | 2 |
| negate_min | Basic | 0 | 1 | 0 | 0 | 0 | 0 | 2 |
