# WAS language

Terms for the language's collections, defaults, contracts, numeric patterns, pipelines,
and local closures. Proposed extensions remain distinct from implemented language support.

## Language

**Temporary candidate**:
An allocation proven to have a bounded lifetime within its allocating invocation,
without publishing its contents' storage for later use. Candidate status alone
neither changes storage nor authorizes reclamation.
_Avoid_: Temporary region allocation, managed heap object

**Number**:
The language's single numeric type, with IEEE-754 binary64 semantics, including
fractional values, signed zero, infinities, and NaN. Integer-valued numbers do not
form a separate language type.
_Avoid_: Inferred integer type, integer-looking type

**Array**:
A mutable collection whose elements share one type; assignment and parameter
passing share the collection rather than copying its elements. Arrays currently
have fixed length; the memory roadmap proposes extending them with resizing.
_Avoid_: List, growable list

**Resizable array**:
A proposed extension of arrays whose length can change while all aliases continue
to refer to the same collection. Resizing is not currently supported.
_Avoid_: Growable list, separate list type

**Pure expression**:
An expression admitted by the WAS default/contract whitelist that performs no writes
or runtime allocation; it may read shared array contents or trap.
_Avoid_: Nontrapping expression, arbitrary side-effect-free expression

**Default expression**:
An expression supplying an omitted argument for an invocation, resolved in the callee's
declaration scope with access only to earlier formal parameters.

**Contract expression**:
A boolean precondition over a named function's formal parameters, evaluated in declaration
order before its body; a false condition causes a trap.

**Interval pattern**:
A numeric match pattern selecting values between constant bounds, including fractional
values. It denotes an interval, not the stepped progression of a range value.
_Avoid_: Range membership pattern, progression pattern

**Pipeline**:
An ordered chain that supplies each preceding result to the first parameter of
the next named user-function, visible local callable, or immediate pipeline stage;
it does not make functions into values.

**Pipeline stage**:
An invocation within a pipeline, receiving the preceding input as its first
argument before any written arguments. Named user-function stages can use defaults,
local callable stages are positional-only, and immediate stages take only the input.

**Final Void stage**:
The last invocation in a pipeline whose declared result is void, permitted only
where the pipeline's result is discarded.

**Local callable binding**:
A lexical name for a closure that can be invoked directly but cannot be aliased,
reassigned, passed, returned, or otherwise used as a value.
_Avoid_: Function-valued local, first-class function

**Shared-binding capture**:
A closure's reference to an original lexical binding, preserving its type and
mutability and observing reassignment from either side.
_Avoid_: Snapshot capture, capture by value

**Lexical callable dependency**:
A closure's dependence on an earlier visible local callable for direct invocation,
including its live context when required. It is not a callable alias or value capture.

**Placeholder region**:
A local callable initializer or immediate pipeline stage in which each
expression-position `_` denotes a distinct anonymous input in source order.
Grouping parentheses do not create another region, and nested callables have
separate boundaries.

**Placeholder callable**:
A callable whose anonymous parameter types are uniquely determined
by constraints within its own placeholder region, not by its callers.
_Avoid_: Partial application, implicit first-class function

**Immediate pipeline stage**:
A parenthesized unary typed lambda or placeholder region invoked directly with
the preceding pipeline input, without producing a callable value. A placeholder
stage's sole parameter type is determined within its region, never by that input.
