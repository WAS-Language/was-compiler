# Derived integrated integer results (#42)

Current cumulative #39/#40/#41: None versus Basic, not isolated incremental pass timing.
Release/regression acceptance **PENDING**; experimental, None default.

Derived by [report.py](report.py) from immutable [collection-1/summary.json](collection-1/summary.json).
All pairs are repetition 1 / 2, paired by repetition (not adjacent command order).
Delta = Basic − None; percent = 100 × (Basic / None − 1). Positive time means slower.
Strict sign in both repeats gives higher_both/lower_both; zeros or reversals give mixed_or_equal.
These classifications are descriptive, not significance tests or accepted regressions.

## Size (all 27 independent modules)

Unweighted sum: **13,414 → 9,538 bytes** (-3,876; -28.90%). 24 shrink, 3 unchanged, 0 grow.
Synthetic corpus sum, not a combined application or production-traffic estimate.

| Case | None bytes | Basic bytes | Delta bytes | Delta % |
| --- | ---: | ---: | ---: | ---: |
| literal_1 | 100 | 75 | -25 | -25.00% |
| locals_1 | 121 | 96 | -25 | -20.66% |
| compare_1 | 196 | 134 | -62 | -31.63% |
| negate_1 | 116 | 93 | -23 | -19.83% |
| literal_32 | 1156 | 542 | -614 | -53.11% |
| locals_32 | 1828 | 1214 | -614 | -33.59% |
| compare_32 | 4164 | 2430 | -1734 | -41.64% |
| negate_32 | 1668 | 1118 | -550 | -32.97% |
| loop_local | 1208 | 1178 | -30 | -2.48% |
| fraction | 70 | 70 | +0 | +0.00% |
| negative_zero | 61 | 55 | -6 | -9.84% |
| negate_zero | 67 | 61 | -6 | -8.96% |
| positive_zero | 70 | 57 | -13 | -18.57% |
| overflow | 80 | 66 | -14 | -17.50% |
| underflow_i32 | 81 | 69 | -12 | -14.81% |
| negate_min | 68 | 68 | +0 | +0.00% |
| unknown | 63 | 57 | -6 | -9.52% |
| mutable | 87 | 75 | -12 | -13.79% |
| capture | 764 | 748 | -16 | -2.09% |
| multiply | 70 | 58 | -12 | -17.14% |
| divide | 70 | 58 | -12 | -17.14% |
| remainder | 236 | 226 | -10 | -4.24% |
| nan | 70 | 58 | -12 | -17.14% |
| infinity | 70 | 58 | -12 | -17.14% |
| subnormal | 60 | 60 | +0 | +0.00% |
| effects | 94 | 68 | -26 | -27.66% |
| trap_order | 776 | 746 | -30 | -3.87% |

## source_compile (µs/compilation)

| Case | None 1 / 2 | Basic 1 / 2 | Delta 1 / 2 | Delta % 1 / 2 | Direction |
| --- | ---: | ---: | ---: | ---: | --- |
| literal_1 | 11.69 / 20.19 | 11.89 / 26.39 | 0.2 / 6.2 | +1.71% / +30.71% | higher_both |
| locals_1 | 14.19 / 27.94 | 16.99 / 37.49 | 2.8 / 9.55 | +19.73% / +34.18% | higher_both |
| compare_1 | 19.34 / 57.09 | 24.49 / 59.89 | 5.15 / 2.8 | +26.63% / +4.90% | higher_both |
| negate_1 | 14.99 / 25.09 | 16.19 / 44.04 | 1.2 / 18.95 | +8.01% / +75.53% | higher_both |
| literal_32 | 91.24 / 212.7 | 123.7 / 286.1 | 32.46 / 73.4 | +35.58% / +34.51% | higher_both |
| locals_32 | 289.9 / 514.2 | 314.1 / 731.5 | 24.2 / 217.3 | +8.35% / +42.26% | higher_both |
| compare_32 | 679.3 / 1484 | 733.2 / 1109 | 53.9 / -375 | +7.93% / -25.27% | mixed_or_equal |
| negate_32 | 261 / 409.8 | 403.8 / 666.7 | 142.8 / 256.9 | +54.71% / +62.69% | higher_both |
| loop_local | 29.89 / 51.49 | 31.49 / 75.09 | 1.6 / 23.6 | +5.35% / +45.83% | higher_both |
| fraction | 7.099 / 13.29 | 7.599 / 11.39 | 0.5 / -1.9 | +7.04% / -14.30% | mixed_or_equal |
| negative_zero | 7.499 / 10.59 | 11.19 / 17.39 | 3.691 / 6.8 | +49.22% / +64.21% | higher_both |
| negate_zero | 9.299 / 12.79 | 13.79 / 22.29 | 4.491 / 9.5 | +48.30% / +74.28% | higher_both |
| positive_zero | 8.049 / 11.39 | 11.69 / 18.89 | 3.641 / 7.5 | +45.24% / +65.85% | higher_both |
| overflow | 9.099 / 12.79 | 13.59 / 15.09 | 4.491 / 2.3 | +49.36% / +17.98% | higher_both |
| underflow_i32 | 9.399 / 13.49 | 14.69 / 22.09 | 5.291 / 8.6 | +56.29% / +63.75% | higher_both |
| negate_min | 9.799 / 13.39 | 14.49 / 23.09 | 4.691 / 9.7 | +47.87% / +72.44% | higher_both |
| unknown | 7.699 / 11.09 | 11.94 / 19.09 | 4.241 / 8 | +55.09% / +72.14% | higher_both |
| mutable | 10.09 / 17.29 | 10.69 / 30.54 | 0.6 / 13.25 | +5.95% / +76.63% | higher_both |
| capture | 20.44 / 58.34 | 26.19 / 53.79 | 5.75 / -4.55 | +28.13% / -7.80% | mixed_or_equal |
| multiply | 7.199 / 12.49 | 7.299 / 20.14 | 0.1 / 7.65 | +1.39% / +61.25% | higher_both |
| divide | 7.399 / 13.19 | 7.299 / 17.94 | -0.1 / 4.75 | -1.35% / +36.01% | mixed_or_equal |
| remainder | 10.19 / 14.19 | 15.09 / 24.29 | 4.9 / 10.1 | +48.09% / +71.18% | higher_both |
| nan | 7.099 / 12.19 | 7.299 / 12.29 | 0.2 / 0.1 | +2.82% / +0.82% | higher_both |
| infinity | 7.099 / 13.19 | 7.399 / 16.64 | 0.3 / 3.45 | +4.23% / +26.16% | higher_both |
| subnormal | 8.799 / 12.59 | 14.19 / 22.24 | 5.391 / 9.65 | +61.27% / +76.65% | higher_both |
| effects | 10.49 / 20.19 | 11.89 / 25.44 | 1.4 / 5.25 | +13.35% / +26.00% | higher_both |
| trap_order | 25.49 / 37.79 | 39.84 / 58.24 | 14.35 / 20.45 | +56.30% / +54.11% | higher_both |

higher_both: 23; lower_both: 0; mixed_or_equal: 4; verify_only: 0.

## typed_compile (µs/compilation)

| Case | None 1 / 2 | Basic 1 / 2 | Delta 1 / 2 | Delta % 1 / 2 | Direction |
| --- | ---: | ---: | ---: | ---: | --- |
| literal_1 | 4.699 / 6.799 | 5.249 / 12.49 | 0.55 / 5.691 | +11.70% / +83.70% | higher_both |
| locals_1 | 6.399 / 9.399 | 11.39 / 13.19 | 4.991 / 3.791 | +78.00% / +40.33% | higher_both |
| compare_1 | 8.299 / 11.99 | 17.44 / 24.34 | 9.141 / 12.35 | +110.15% / +103.00% | higher_both |
| negate_1 | 5.999 / 8.699 | 10.34 / 10.99 | 4.341 / 2.291 | +72.36% / +26.34% | higher_both |
| literal_32 | 25.49 / 37.69 | 42.84 / 82.29 | 17.35 / 44.6 | +68.07% / +118.33% | higher_both |
| locals_32 | 67.79 / 101.6 | 115 / 166.3 | 47.21 / 64.7 | +69.64% / +63.68% | higher_both |
| compare_32 | 127.3 / 185.5 | 308.4 / 445.7 | 181.1 / 260.2 | +142.26% / +140.27% | higher_both |
| negate_32 | 67.54 / 91.84 | 151.3 / 155.4 | 83.76 / 63.56 | +124.02% / +69.21% | higher_both |
| loop_local | 20.59 / 30.79 | 22.79 / 34.09 | 2.2 / 3.3 | +10.68% / +10.72% | higher_both |
| fraction | 3.499 / 5.099 | 3.599 / 9.348 | 0.1 / 4.249 | +2.86% / +83.33% | higher_both |
| negative_zero | 3.599 / 4.799 | 4.999 / 4.998 | 1.4 / 0.199 | +38.90% / +4.15% | higher_both |
| negate_zero | 4.249 / 5.499 | 6.149 / 6.198 | 1.9 / 0.699 | +44.72% / +12.71% | higher_both |
| positive_zero | 3.999 / 5.299 | 5.699 / 7.898 | 1.7 / 2.599 | +42.51% / +49.05% | higher_both |
| overflow | 4.199 / 5.699 | 6.349 / 6.748 | 2.15 / 1.049 | +51.20% / +18.41% | higher_both |
| underflow_i32 | 4.599 / 5.999 | 7.149 / 6.498 | 2.55 / 0.499 | +55.45% / +8.32% | higher_both |
| negate_min | 4.399 / 5.599 | 5.899 / 6.598 | 1.5 / 0.999 | +34.10% / +17.84% | higher_both |
| unknown | 3.999 / 5.299 | 6.099 / 8.298 | 2.1 / 2.999 | +52.51% / +56.60% | higher_both |
| mutable | 5.799 / 6.299 | 6.949 / 7.198 | 1.15 / 0.899 | +19.83% / +14.27% | higher_both |
| capture | 17.94 / 25.64 | 26.44 / 41.64 | 8.5 / 16 | +47.38% / +62.40% | higher_both |
| multiply | 3.299 / 4.999 | 3.249 / 5.198 | -0.05 / 0.199 | -1.52% / +3.98% | mixed_or_equal |
| divide | 3.499 / 5.199 | 5.899 / 5.898 | 2.4 / 0.699 | +68.59% / +13.44% | higher_both |
| remainder | 6.099 / 8.299 | 9.199 / 8.798 | 3.1 / 0.499 | +50.83% / +6.01% | higher_both |
| nan | 3.399 / 4.949 | 5.399 / 5.098 | 2 / 0.149 | +58.84% / +3.01% | higher_both |
| infinity | 3.499 / 5.099 | 3.449 / 9.448 | -0.05 / 4.349 | -1.43% / +85.29% | mixed_or_equal |
| subnormal | 4.299 / 5.699 | 6.799 / 6.598 | 2.5 / 0.899 | +58.15% / +15.77% | higher_both |
| effects | 4.199 / 6.099 | 4.999 / 13.89 | 0.8 / 7.791 | +19.05% / +127.74% | higher_both |
| trap_order | 18.09 / 24.29 | 26.29 / 26.39 | 8.2 / 2.1 | +45.33% / +8.65% | higher_both |

higher_both: 25; lower_both: 0; mixed_or_equal: 2; verify_only: 0.

## guest (ns/main call)

| Case | None 1 / 2 | Basic 1 / 2 | Delta 1 / 2 | Delta % 1 / 2 | Direction |
| --- | ---: | ---: | ---: | ---: | --- |
| literal_1 | 87.5 / 93.75 | 92.1875 / 92.9688 | 4.6875 / -0.78125 | +5.36% / -0.83% | mixed_or_equal |
| locals_1 | 76.5625 / 79.6875 | 75.7812 / 71.875 | -0.78125 / -7.8125 | -1.02% / -9.80% | lower_both |
| compare_1 | 73.4375 / 76.5625 | 82.0312 / 74.2188 | 8.59375 / -2.34375 | +11.70% / -3.06% | mixed_or_equal |
| negate_1 | 68.75 / 67.9688 | 71.0938 / 71.875 | 2.34375 / 3.90625 | +3.41% / +5.75% | higher_both |
| literal_32 | 95.3125 / 92.1875 | 81.25 / 81.25 | -14.0625 / -10.9375 | -14.75% / -11.86% | lower_both |
| locals_32 | 120.312 / 121.875 | 126.562 / 124.219 | 6.25 / 2.34375 | +5.19% / +1.92% | higher_both |
| compare_32 | 382.812 / 382.031 | 189.844 / 195.312 | -192.969 / -186.719 | -50.41% / -48.88% | lower_both |
| negate_32 | 126.562 / 123.438 | 110.938 / 117.188 | -15.625 / -6.25 | -12.35% / -5.06% | lower_both |
| loop_local | 302.344 / 325.781 | 355.469 / 355.469 | 53.125 / 29.6875 | +17.57% / +9.11% | higher_both |
| fraction | 70.3125 / 72.6562 | 77.3438 / 73.4375 | 7.03125 / 0.78125 | +10.00% / +1.08% | higher_both |
| negative_zero | 68.75 / 67.1875 | 76.5625 / 73.4375 | 7.8125 / 6.25 | +11.36% / +9.30% | higher_both |
| negate_zero | 67.1875 / 65.625 | 78.125 / 71.875 | 10.9375 / 6.25 | +16.28% / +9.52% | higher_both |
| positive_zero | 67.1875 / 64.0625 | 77.3438 / 67.9688 | 10.1562 / 3.90625 | +15.12% / +6.10% | higher_both |
| overflow | 63.2812 / 70.3125 | 71.875 / 70.3125 | 8.59375 / 0 | +13.58% / +0.00% | mixed_or_equal |
| underflow_i32 | 69.5312 / 67.1875 | 73.4375 / 70.3125 | 3.90625 / 3.125 | +5.62% / +4.65% | higher_both |
| negate_min | 66.4062 / 68.75 | 67.9688 / 70.3125 | 1.5625 / 1.5625 | +2.35% / +2.27% | higher_both |
| unknown | 66.4062 / 67.1875 | 62.5 / 67.9688 | -3.90625 / 0.78125 | -5.88% / +1.16% | mixed_or_equal |
| mutable | 64.0625 / 67.1875 | 64.0625 / 70.3125 | 0 / 3.125 | +0.00% / +4.65% | mixed_or_equal |
| capture | 92.1875 / 93.75 | 93.75 / 98.4375 | 1.5625 / 4.6875 | +1.69% / +5.00% | higher_both |
| multiply | 64.0625 / 67.1875 | 65.625 / 68.75 | 1.5625 / 1.5625 | +2.44% / +2.33% | higher_both |
| divide | 65.625 / 64.0625 | 64.8438 / 71.0938 | -0.78125 / 7.03125 | -1.19% / +10.98% | mixed_or_equal |
| remainder | 114.062 / 96.875 | 89.0625 / 100 | -25 / 3.125 | -21.92% / +3.23% | mixed_or_equal |
| nan | 67.1875 / 67.1875 | 65.625 / 70.3125 | -1.5625 / 3.125 | -2.33% / +4.65% | mixed_or_equal |
| infinity | 65.625 / 67.9688 | 64.8438 / 71.875 | -0.78125 / 3.90625 | -1.19% / +5.75% | mixed_or_equal |
| subnormal | 67.1875 / 67.1875 | 64.8438 / 71.0938 | -2.34375 / 3.90625 | -3.49% / +5.81% | mixed_or_equal |
| effects | 275.781 / 178.125 | 203.906 / 185.938 | -71.875 / 7.8125 | -26.06% / +4.39% | mixed_or_equal |
| trap_order | verify-only / verify-only | verify-only / verify-only | verify-only / verify-only | verify-only / verify-only | verify_only |

higher_both: 11; lower_both: 4; mixed_or_equal: 11; verify_only: 1.

## Exact historical identities

All 27 current None source/module identities equal #38 and #41 None; all 27 current Basic
source/module identities equal #41 Basic, including byte lengths and full SHA-256 hashes.
Current None/Basic source identities also match in all 27 cases. Thus size delta versus #41
is exactly zero at both levels. Historical timing is not used as a matched control.
Full historical and current identities are retained per case in [results.json](results.json).

| Case | Source SHA-256 (all compared records) | None module SHA-256 (#38/#41/current) | Basic module SHA-256 (#41/current) |
| --- | --- | --- | --- |
| literal_1 | `1e4907943692fd5dc1b37d13c7cb44fc74b001843c884a26e84322b00a7af828` | `95d4855ba19fd5fa3dd4df743c66c28f760677eb1178e968774fe20af09b6e03` | `2230921937f8c34add2e04a3e4924b315bf282ac9cf518acac83c46e93b01b2f` |
| locals_1 | `208264adedf68278fdc28274729574f4fb52a90b63ccbd6d30eb1d9a09694a48` | `6227f3aea449a2fcb3c22ba44389e09cb0b31b4fea6b189daf42a827bb3f53b2` | `e9d86f57b25751c111ff6d6f33ab1047b925e120e84cfc7f158979d049f4c252` |
| compare_1 | `fd0ddea2e782aeb980e25dcbff5b4379e748fff9e5ff50e2e33c87840a883608` | `7c3ddf34cb3fca6bbb1bb827a5e3270a812acc649baaa16b05e702e391d6f065` | `c5c012e642e41ab3f9b21aa9d1f666c8dc50549ffe4618de4ed5a420a9522bb9` |
| negate_1 | `a7caf71b5b245ed786cc7be90932993a9cc520889bd65ef9818237e67f259cc4` | `7d39f7080239c8acb9fe404c0b9adb41b95ea507a446fdea93310d16c68eea4a` | `9ed8113a003dcb90a5cf9d769c8d003fd505ced61648bcfcb79df62de0a13d0e` |
| literal_32 | `786242ca5cbe0b752ede95ccd824b18201266feb14e82a9831e0106fe1abc114` | `4d012f3c178294028cca89c1827672286806a1aa4d3ae387c456c00512f3108e` | `a2fc821e5b5131fec86817f6914b36ada61b140910bdccd87859e73aaf248970` |
| locals_32 | `1297e702b9ade61550023c6201b23f135c695d3e82f2e5b049b3e0edf930b37d` | `cdfcecc1a1dc4a7060c75125c0ef92fb3dc925b83a97421fcb2421c0276ebba6` | `c4e97cac21610ba6365510bbb2e33d0d5ec42e0973d007f25cf63de9b3cf335c` |
| compare_32 | `7cd33a427f17d1c3b842ca5ea33cca6fc0169d21343d6228f738cb2ea2514088` | `534138e87c7870588ac5193c9d2e60c7066fd37b5b79de84b615b11e0824e71e` | `ef8f7320a176410c3b00023d506a07fecc69d07938a990229d4e0b2f54a6f2d8` |
| negate_32 | `6cd142cd1b5a94c4ff044ab1e60ec25fb0b309251361bd065bc8c26aadcfb9fe` | `a01608b6b906434380e940d6892e665217dc3b0783e3c393063fbc3873fb49f5` | `cf862ad9eb09d0b99098463ed206cd653bee7b2dc07ba1244377df8d73ec5c6f` |
| loop_local | `5f48abafe8f2da6f2c51669802a028ac9eabc334f56e4923447197c694b24c3f` | `6bbe520aeefe5d9428654a10beec937d5107c90f8ac5b32e7a21e606e51f5981` | `d556befc96c8108b9328bd0b1d7268c3eaa5933bbb6d11e3629c9ff4e74a6fac` |
| fraction | `bf07434ca629c6ca644c019df4aa228d927dba54cd1c76e4e34172f8319dc1dc` | `f44f0944e9f462642489c60165221f2953173dd25d13604f299609365a0a631d` | `f44f0944e9f462642489c60165221f2953173dd25d13604f299609365a0a631d` |
| negative_zero | `c4a1d6ea6386aa074f17d138d126a5531dd1f649b126b5d805fb0f80f4aaa669` | `661fef4dc924f036a7aee1fcd718dbe42d43c26dfaf29f4b0748cf22fcd56478` | `c0894233f443ad1dd5ffe4b5c0e2b842c1e60f21fabc4da664dbbb2dd3eafd60` |
| negate_zero | `70cfe8a79a4b8a6ef87f82fb865c1970d51f14eeb653766fa834b021b2e49941` | `add8abceea976ca66ea297e8cead050d41e3931b3e9ab5dcebb329136f98a7b0` | `7da10746c0dbfacf24f8f2cfca3bca0ee8e22def13033756b7a1596ea1b0b4dd` |
| positive_zero | `0461dfeeba4fe3da20bdf68f81d4dcad6878f90523d5657999041e5ed5c74ca9` | `224a8ae00680b480f2c6cce5a1a15784703a5a13c188cb0adb9b8a6a00787999` | `97b90e24b4fde5601501445fe16fe6f1c4025995a7afa065c402ca76ca248578` |
| overflow | `f9de676f3e6c9c74a9f1f5b26be4c147a8565c61d3da7a83d8d67f37a1e5b41f` | `e63deb9876f721163b26132af4dc318caf7403cb65f0313d5d3d47bfa676f9d3` | `7595416085057cd9b83a0990e373b5eb5ce320e7b3582ededa8122d85a08130e` |
| underflow_i32 | `1cb649f1a0d1bb5b7280d70ad57e7447ccb54df657748ec58129a3b40d265452` | `85b85dc922415b1af7c5aa5eb299dcdcbd41a00fd3a6370fab7f4db62756b3ca` | `49e1d47725c7cf2c33c8ebd31f799b4d03977d61bdde4efb699f79d68154375d` |
| negate_min | `0b37109cae2424d03298a71d373f0f65d64c8380e544b39fd8503bc781e3a3a4` | `06ac94519893972834b38e6801cf1585c9e7c3df07633dda5960861bccbf7473` | `06ac94519893972834b38e6801cf1585c9e7c3df07633dda5960861bccbf7473` |
| unknown | `3c26e0ddea1824c288952d7b4299cced2fcf2abcf69ac5b2eed99243c6b3bbbd` | `7769a4f0de51c5e4891a84741801f8ce61bb8801703a1cbbe639140f8c6e7e97` | `5307f7288e4a6b98b4ae239abd5629184e323cc8e6b339023b0b9a0e3b1d0632` |
| mutable | `0f67f39788bbe4476e02e25272c9ad2e92934d160a23c2de1ccb6c8dd4283c56` | `aecd6f575e92ff95fcef5df218fb4c1bed83cad7f314974300fccaaf6f09dc90` | `a88426da69a5669e05befe41040d9d843e63c85fcc01ba4b44f13774081c4863` |
| capture | `d9ade799a4bcaad6f3d2a4d31cdf83e7f415c61f6281b05227d2bf9d2ca3378e` | `38b8730c9253291798bf4799e39c2cab3c203ef38d29503ff6caecd4759dc8a7` | `b3a03f5a90aa0dc9479f543ab6656fd666c98fbb2f4ceaf0df647a9f31b42c7e` |
| multiply | `127a8c57c614b2390fb42ee1168132aac445386e8e5c2a7ae44df8db0330649f` | `2bc3412461d81fa49fd2cc449180120dd54a2e86f50d198e11a1cefb5d5c0d23` | `e1492a01aa68d608c63ef830f35a6555994391d0fa22a93150410a02d4fbaa20` |
| divide | `a19d5b67330a4c0884016832aaa64410751ea5767bfec263d3a83b88f16c1166` | `894679ede9c5251b1900207c77c8c739d7fd44a50de27a755383987d78336060` | `9dcdf98da1f41e167153d6dc178e0acac3502fed8165d1c6e19a74526b49ab27` |
| remainder | `cab3b38fa1a93d43ce232e2329de5cb7a863b0c0f73f80077bdea6efaa61be27` | `574aa747d3a3603e6742f8d66e814603a5c681cdb255a457a3b3794b796a6b19` | `5c30c2212dd59440ecd0366c0a86fe59b3e9a9c2e07de7fca8700c038a674a3b` |
| nan | `386ba7229ec2efc0c7e0052eed6ac52771dfded1d70c2a5d545dc1cd40d39f79` | `20748c5766f2395bbfb11c388412f65a52363da62a66c3b6da1d8159e00ed856` | `3364f4811c90001b472d049c9f677eac0902deb344179220db1792f793301218` |
| infinity | `da3ed447d0325746a8c027e2e7bd3863cbb4acefe7ec95cfd467184be53c0afe` | `5f0010eec6a2d727426ae124ca6dc24d4782a34e815d0826df0f809715e57a4a` | `bc2a58030263f62b33170dd8f90adb377ea9058bcea6ba27b27976f5cd6abe8c` |
| subnormal | `46c5af5aa716c0b4dc5f1bce9ca7bcbac83307f86dcab3d699cf4fa0ab1425ec` | `62b89ca128870326af0dba5391a458b3836efa5c7157a32d81d8c6a6a90a8372` | `62b89ca128870326af0dba5391a458b3836efa5c7157a32d81d8c6a6a90a8372` |
| effects | `83e3a27ce9f43a06b2f6d3dba7f63273ac74b9f31262095fc20a5460d9e491f4` | `5749d8cd715df5ea5f1965f9a1c8c7443f3615129e2185c41a34e7a11dca4f90` | `a77236d5ef91f00731f60fe691b56c3092dc74263bcb91b84097a7ad1bc511be` |
| trap_order | `11c29c715f492c6c91990cc423a15ac93f69a92b5f7699f929b2b69acea35981` | `0416b14d9f6f1d16d1126d72ef1fe30ac16ba0bdb5d8d02cc4003496030236fd` | `4483a59a8b1fc04c3c4bf18a6ee61c10358ff22dbcf58ca929779767b53983ed` |

## Provenance and precision

Measured base HEAD: `0e1b5225a6d63c18244ab8c06fc460dd2d7a0020` plus the recorded dirty test-only diff; see README.
Compiler values retain the recorder's parsed Divan min/max/median/mean aggregates in results.json;
100 samples × one compilation, OS timer. Raw per-compilation samples were not recorded.
Guest logs retain 20 raw ns/main-call samples (64 calls/sample), after 128 warmup calls;
trap_order is verify-only. Neither raw guest samples nor Divan aggregates are interchangeable.
See [README](README.md) for boundaries, limitations, commands and conversion tradeoffs.
results.json includes all recorded command metadata, environment, source hashes and evidence-file hashes.
No new validation runs or release claims are made by this derivation.
