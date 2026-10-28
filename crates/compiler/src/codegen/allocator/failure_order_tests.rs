use super::{Config, compile};
use crate::{Compiler, exports::ExportOptions};
use wasmparser::{BlockType, ExternalKind, Operator, Parser, Payload, TypeRef, Validator};

struct Decoded<'a> {
    imports: usize,
    exports: Vec<(&'a str, u32)>,
    bodies: Vec<Vec<Operator<'a>>>,
}

impl<'a> Decoded<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Validator::new().validate_all(bytes).unwrap();
        let mut decoded = Self {
            imports: 0,
            exports: Vec::new(),
            bodies: Vec::new(),
        };

        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        if matches!(import.unwrap().ty, TypeRef::Func(_)) {
                            decoded.imports += 1;
                        }
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.unwrap();
                        if export.kind == ExternalKind::Func {
                            decoded.exports.push((export.name, export.index));
                        }
                    }
                }
                Payload::CodeSectionEntry(body) => {
                    decoded.bodies.push(
                        body.get_operators_reader()
                            .unwrap()
                            .into_iter()
                            .map(Result::unwrap)
                            .collect(),
                    );
                }
                _ => {}
            }
        }
        decoded
    }

    fn index(&self, name: &str) -> u32 {
        self.exports
            .iter()
            .find_map(|(export, index)| (*export == name).then_some(*index))
            .unwrap_or_else(|| panic!("missing function export {name}"))
    }

    fn body(&self, index: u32) -> &[Operator<'a>] {
        &self.bodies[usize::try_from(index).unwrap() - self.imports]
    }
}

fn fixture(backing_limit: Option<u64>) -> Vec<u8> {
    let program = Compiler::new()
        .analyze("fn main() -> number[] { return [1]; }")
        .unwrap();
    compile(
        &program,
        &ExportOptions::All,
        Config {
            last_identity: 0,
            backing_limit,
        },
    )
    .unwrap()
}

fn unique_window(
    ops: &[Operator<'_>],
    width: usize,
    predicate: impl Fn(&[Operator<'_>]) -> bool,
) -> usize {
    let positions: Vec<_> = ops
        .windows(width)
        .enumerate()
        .filter_map(|(position, window)| predicate(window).then_some(position))
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "expected one matching {width}-op window"
    );
    positions[0]
}

#[test]
fn decoded_classify_checks_signed_request_domain_before_any_narrowing_or_state_access() {
    use Operator::*;

    let bytes = fixture(None);
    let module = Decoded::new(&bytes);
    let ops = module.body(module.index("__m3_class"));
    assert!(matches!(
        ops,
        [
            LocalGet { local_index: 0 },
            I64Const { value: 0 },
            I64LtS,
            LocalGet { local_index: 0 },
            I64Const {
                value: 2_147_483_648
            },
            I64GtU,
            I32Or,
            If {
                blockty: BlockType::Empty
            },
            Unreachable,
            End,
            I64Const { value: 8 },
            ..
        ]
    ));
    // A closed instruction set rules out hidden calls, narrowing, memory access
    // and global access anywhere in this nonmutating classification helper.
    assert!(ops.iter().all(|op| matches!(
        op,
        LocalGet { .. }
            | LocalSet { .. }
            | I64Const { .. }
            | I32Const { .. }
            | I64LtS
            | I64GtU
            | I32Or
            | I64GeU
            | I64Shl
            | I32Add
            | If {
                blockty: BlockType::Empty
            }
            | Block {
                blockty: BlockType::Empty
            }
            | Loop {
                blockty: BlockType::Empty
            }
            | BrIf { .. }
            | Br { .. }
            | Unreachable
            | End
    )));
}

#[test]
fn decoded_acquire_validates_request_and_identity_before_reading_or_removing_free_head() {
    use Operator::*;

    let bytes = fixture(None);
    let module = Decoded::new(&bytes);
    let ops = module.body(module.index("__m3_acquire"));
    let [
        LocalGet { local_index: 0 },
        Call {
            function_index: classify,
        },
        LocalSet { local_index: class },
        GlobalGet { global_index: last },
        I64Const { value: -1 },
        I64Eq,
        If {
            blockty: BlockType::Empty,
        },
        Unreachable,
        End,
        GlobalGet {
            global_index: renewed_last,
        },
        I64Const { value: 1 },
        I64Add,
        LocalSet {
            local_index: identity,
        },
        I64Const { value: 8 },
        LocalGet {
            local_index: shifted_class,
        },
        I64ExtendI32U,
        I64Shl,
        LocalSet {
            local_index: capacity,
        },
        LocalGet {
            local_index: head_class,
        },
        Call {
            function_index: head,
        },
        LocalTee {
            local_index: record,
        },
        I32Eqz,
        If {
            blockty: BlockType::Empty,
        },
        ..,
    ] = ops
    else {
        panic!(
            "request classification and u64::MAX trap must precede all free-list/backing access"
        );
    };
    assert_eq!(*classify, module.index("__m3_class"));
    assert_eq!(last, renewed_last);
    assert_eq!(class, shifted_class);
    assert_eq!(class, head_class);
    assert_ne!(capacity, identity);
    assert_ne!(class, record);

    // Resolve the actual called head helper rather than trusting an emitter slot.
    let head_ops = module.body(*head);
    assert_eq!(
        head_ops
            .iter()
            .filter(|op| matches!(op, GlobalGet { .. }))
            .count(),
        29
    );
    assert!(head_ops.iter().all(|op| matches!(
        op,
        LocalGet { .. }
            | I32Const { .. }
            | I32Eq
            | If {
                blockty: BlockType::Empty
            }
            | GlobalGet { .. }
            | Return
            | End
            | Unreachable
    )));
    let calls: Vec<_> = ops
        .iter()
        .filter_map(|op| match op {
            Call { function_index } => Some(*function_index),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls.len(),
        4,
        "classify, head read, backing, head removal only"
    );
    assert_eq!(calls[0], *classify);
    assert_eq!(calls[1], *head);
    assert_eq!(calls[2], module.index("__alloc"));
    assert_ne!(calls[3], *head);
}

#[test]
fn decoded_fresh_reservation_checks_widened_size_before_backing_and_commits_after_success() {
    use Operator::*;

    // Exercise both emitted configurations: an optional ceiling must not replace
    // the unconditional size bound or the real checked backing reservation.
    for limit in [None, Some(4_294_901_760)] {
        let bytes = fixture(limit);
        let module = Decoded::new(&bytes);
        let ops = module.body(module.index("__m3_acquire"));
        let sizing = unique_window(ops, 12, |window| {
            matches!(window,
                [If { blockty: BlockType::Empty }, LocalGet { .. }, I64Const { value: 32 },
                    I64Add, LocalSet { local_index: total }, LocalGet { local_index: checked },
                    I64Const { value: 4_294_967_295 }, I64GtU,
                    If { blockty: BlockType::Empty }, Unreachable, End, LocalGet { .. }]
                    if total == checked && limit.is_none()
            ) || matches!(window,
                [If { blockty: BlockType::Empty }, LocalGet { .. }, I64Const { value: 32 },
                    I64Add, LocalSet { local_index: total }, LocalGet { local_index: checked },
                    I64Const { value: 4_294_967_295 }, I64GtU,
                    If { blockty: BlockType::Empty }, Unreachable, End, GlobalGet { global_index: 0 }]
                    if total == checked && limit.is_some()
            )
        });
        let LocalSet { local_index: total } = ops[sizing + 4] else {
            unreachable!()
        };
        let LocalGet {
            local_index: capacity,
        } = ops[sizing + 1]
        else {
            unreachable!()
        };
        assert!(matches!(ops[sizing - 5..sizing - 3],
            [LocalSet { local_index }, LocalGet { .. }] if local_index == capacity
        ));
        let reservation = unique_window(ops, 4, |window| {
            matches!(window,
                [LocalGet { local_index }, I32WrapI64, Call { function_index }, LocalSet { .. }]
                    if *local_index == total && *function_index == module.index("__alloc")
            )
        });
        let checked_end = sizing + 11;
        if limit.is_some() {
            assert!(matches!(&ops[checked_end..reservation],
                [GlobalGet { global_index: 0 }, I64ExtendI32U,
                    LocalGet { local_index }, I64Add, I64Const { value: 4_294_901_760 },
                    I64GtU, If { blockty: BlockType::Empty }, Unreachable, End]
                    if *local_index == total
            ));
        } else {
            assert_eq!(checked_end, reservation);
        }
        assert!(ops[..reservation].iter().all(|op| !matches!(
            op,
            I32WrapI64
                | GlobalSet { .. }
                | I32Store { .. }
                | I64Store { .. }
                | I32Load { .. }
                | I64Load { .. }
                | MemoryGrow { .. }
        )));

        let LocalSet {
            local_index: record,
        } = ops[reservation + 3]
        else {
            unreachable!()
        };
        let tail = &ops[reservation + 4..];
        let [
            LocalGet {
                local_index: link_record,
            },
            GlobalGet {
                global_index: registry,
            },
            I32Store {
                memarg: registry_link,
            },
            LocalGet {
                local_index: class_record,
            },
            LocalGet { local_index: class },
            I32Store {
                memarg: class_field,
            },
            Else,
            LocalGet {
                local_index: remove_class,
            },
            LocalGet {
                local_index: free_record,
            },
            I32Load { memarg: free_link },
            Call {
                function_index: remove_head,
            },
            End,
            LocalGet {
                local_index: clear_record,
            },
            I32Const { value: 0 },
            I32Store { memarg: clear_link },
            LocalGet {
                local_index: active_record,
            },
            I32Const { value: 1 },
            I32Store { memarg: active },
            LocalGet {
                local_index: identity_record,
            },
            LocalGet {
                local_index: identity,
            },
            I64Store {
                memarg: identity_field,
            },
            LocalGet {
                local_index: request_record,
            },
            LocalGet { local_index: 0 },
            I64Store { memarg: requested },
            LocalGet {
                local_index: fresh_total,
            },
            I64Eqz,
            I32Eqz,
            If {
                blockty: BlockType::Empty,
            },
            LocalGet {
                local_index: publish_record,
            },
            GlobalSet {
                global_index: published_registry,
            },
            GlobalGet {
                global_index: population,
            },
            I64Const { value: 1 },
            I64Add,
            GlobalSet {
                global_index: updated_population,
            },
            End,
            LocalGet {
                local_index: committed_identity,
            },
            GlobalSet { global_index: last },
            LocalGet {
                local_index: payload_record,
            },
            I32Const { value: 32 },
            I32Add,
            LocalGet {
                local_index: returned_identity,
            },
            End,
        ] = tail
        else {
            panic!(
                "fresh initialization, reuse removal and publication must follow successful reservation"
            );
        };
        for local in [
            link_record,
            class_record,
            free_record,
            clear_record,
            active_record,
            identity_record,
            request_record,
            publish_record,
            payload_record,
        ] {
            assert_eq!(*local, record);
        }
        assert_eq!(class, remove_class);
        assert_eq!(*fresh_total, total);
        assert_eq!(registry, published_registry);
        assert_eq!(population, updated_population);
        assert_eq!(identity, committed_identity);
        assert_eq!(identity, returned_identity);
        assert!(matches!(ops[3], GlobalGet { global_index } if global_index == *last));
        for (field, offset, align) in [
            (registry_link, 0, 2),
            (class_field, 8, 2),
            (free_link, 4, 2),
            (clear_link, 4, 2),
            (active, 12, 2),
            (identity_field, 16, 3),
            (requested, 24, 3),
        ] {
            assert_eq!(
                (field.offset, field.align, field.memory),
                (offset, align, 0)
            );
        }
        // Removal is confined to Else (reuse), and no fallible calls follow it.
        let removal = module.body(*remove_head);
        assert_eq!(
            removal
                .iter()
                .filter(|op| matches!(op, GlobalSet { .. }))
                .count(),
            29
        );
        assert!(removal.iter().all(|op| matches!(
            op,
            LocalGet { .. }
                | I32Const { .. }
                | I32Eq
                | If {
                    blockty: BlockType::Empty
                }
                | GlobalSet { .. }
                | Return
                | End
                | Unreachable
        )));
    }
}

#[test]
fn decoded_real_backing_checks_widened_end_and_growth_failure_before_cursor_commit() {
    use Operator::*;

    let bytes = fixture(None);
    let module = Decoded::new(&bytes);
    let ops = module.body(module.index("__alloc"));
    // Match the entire real backing body: no earlier cursor write, wrapped-end
    // comparison, unguarded growth result, or hidden call can satisfy this oracle.
    assert!(matches!(ops,
        [GlobalGet { global_index: 0 }, I64ExtendI32U, LocalTee { local_index: start },
            I64Const { value: 8 }, I64LtU, LocalGet { local_index: aligned_start },
            I64Const { value: 7 }, I64And, I64Const { value: 0 }, I64Ne, I32Or,
            If { blockty: BlockType::Empty }, Unreachable, End,
            LocalGet { local_index: 0 }, I64ExtendI32U, I64Const { value: 7 }, I64Add,
            I64Const { value: -8 }, I64And, LocalTee { local_index: size }, I64Eqz,
            If { blockty: BlockType::Empty }, I64Const { value: 8 },
            LocalSet { local_index: minimum_size }, End,
            LocalGet { local_index: end_start }, LocalGet { local_index: end_size }, I64Add,
            LocalTee { local_index: end }, I64Const { value: 4_294_901_760 }, I64GtU,
            If { blockty: BlockType::Empty }, Unreachable, End,
            LocalGet { local_index: page_end }, I64Const { value: 65_535 }, I64Add,
            I64Const { value: 16 }, I64ShrU, LocalTee { local_index: pages },
            MemorySize { mem: 0 }, I64ExtendI32U, I64GtU, If { blockty: BlockType::Empty },
            LocalGet { local_index: growth_pages }, MemorySize { mem: 0 }, I64ExtendI32U,
            I64Sub, I32WrapI64, MemoryGrow { mem: 0 }, I32Const { value: -1 }, I32Eq,
            If { blockty: BlockType::Empty }, Unreachable, End, End,
            LocalGet { local_index: committed_end }, I32WrapI64, GlobalSet { global_index: 0 },
            LocalGet { local_index: returned_start }, I32WrapI64, End]
            if start == aligned_start && start == end_start && start == returned_start
                && size == minimum_size && size == end_size
                && end == page_end && end == committed_end && pages == growth_pages
    ));
}
