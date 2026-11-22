#[path = "../benches/support/m6_corpus.rs"]
mod corpus;

#[test]
fn measurement_modules_keep_the_allocated_baseline_and_preset_parity() {
    for (workload, source) in corpus::CASES {
        corpus::prepare(workload, source);
    }
}
