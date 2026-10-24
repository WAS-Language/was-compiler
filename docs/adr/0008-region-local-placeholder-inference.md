# Keep placeholder inference inside explicit callable regions

WP12's design interview selected region-local exact constraint inference over an
operator-only sugar pass or caller-driven whole-program inference. This supports
ordinary inferred locals, assignments and joins without making callers or pipeline
inputs determine a callable's signature. It costs a bounded semantic constraint
solver and explicit reachable-evidence rules, but keeps inference variables out of
the public typed AST and leaves the existing checked callable backend intact.

Grouping parentheses do not form regions; only permitted initializers and immediate
stages own placeholders, with nested callable boundaries kept separate. Count
parameters structurally, infer from reachable constraint sites, then check all
children against concrete types. This avoids both accidental callable values and
an implementation-dependent arithmetic whitelist.

See the [design handoff](../wp12-placeholder-development.md) for the accepted
interview answers and examples. Final shared understanding was confirmed in Q26.
This decision does not authorize WP12 implementation or claim release validation.
