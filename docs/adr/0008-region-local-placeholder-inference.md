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

During #46 implementation the user approved a syntax-directed clarification:
bare `_` local initializers inside a placeholder region connect ordinary locals
to that region's parameters. Compound placeholder initializers and explicit
lambdas remain separate callable boundaries. Structural discovery applies the
same rule before inference. This resolves the conflict between #46's required
`{ let x = _; x += 1; x }` example and the original blanket initializer boundary,
without making ownership depend on later uses. A standalone `let identity = _;`
still declares a callable whose parameter is ambiguous without internal evidence.

See the [design handoff](../wp12-placeholder-development.md) for the accepted
interview answers and examples. Final shared understanding was confirmed in Q26.
This decision does not authorize WP12 implementation or claim release validation.
