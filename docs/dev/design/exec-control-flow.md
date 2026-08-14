# Exec program control flow

Engine-internal design: no user-facing API or driver contract changes. The
template's `User-facing API`, `Driver integration`, `Behavior`, and `Edge
cases` sections are omitted — the only observable effect is that some plans
issue fewer driver operations; runtime behavior and edge cases are covered
by `Design` and `Risks`.

## Summary

The exec interpreter runs every action in the compiled plan unconditionally.
This makes it unsafe for a pure read to correlate on data that may not exist
at runtime: the `DO NOTHING` upsert with an eager `belongs_to` runs the
relation-load `SELECT` on the conflict path, where the upsert's `RETURNING`
produced zero rows to correlate on. The engine currently works around it by
feeding the correlation from a `Const` of the insert VALUES
(`back_ref_input` in `plan/statement.rs`).

This design adds conditional execution to the exec program. The
architecture documentation already frames the compiled plan as a mini
program run by a virtual machine, "though there is no control flow (yet)"
(`docs/dev/architecture/query-engine.md`); this is the first control-flow
construct. A structured `If` action wraps the actions that are only
meaningful when a guard condition holds — for the motivating case, "the
upsert returned at least one row" — and the interpreter skips the block
when it does not:

```text
$0 = ExecStatement(INSERT .. ON CONFLICT DO NOTHING RETURNING id, title, user_id)
if non_empty($0):
    $1 = Project($0 -> [user_id])
    $2 = ExecStatement(SELECT .. FROM users WHERE id = $1[0][0])
else:
    $1 = SetVar([])
    $2 = SetVar([])
$3 = Eval(map $0 rows -> Post record, user slot from $2)
return $3
```

SQLite and its Rust reimplementation Turso are the model. Turso's planner
assigns every non-FROM-clause subquery an evaluation phase — a subquery in
a DML RETURNING clause gets `SubqueryEvalPhase::PostWriteReturning`
(`core/translate/plan.rs`) and its bytecode is emitted in the per-row
post-write region; the `DO NOTHING` conflict branch is a `Goto` that jumps
over the write and that region (`core/translate/insert.rs`). Conditionality
is code placement plus a branch, decided statically by the planner. Toasty
adopts the same discipline at its own granularity: coarse actions and a
structured block instead of hundreds of fine instructions and jump offsets.

## Motivation

The compiled plan for

```rust
Post::upsert_by_id(post_id)
    .title("hello")
    .user_id(user.id)   // Post has an eager `user: User`
    .or_ignore()
    .exec(&mut db)
```

is today:

```text
$0 = ExecStatement(INSERT .. ON CONFLICT DO NOTHING RETURNING id, title, user_id)
$1 = Project($0 -> [user_id])          // back-ref feed
$2 = ExecStatement(SELECT .. FROM users WHERE id = $1[0][0])
$3 = Eval(map $0 rows -> Post record, user slot from $2)
```

On conflict `$0` returns zero rows, `$3` maps over an empty list and never
references `$2` — yet `$2` still executes, and its filter substitution
reads row 0 of an empty `$1`. Before the `back_ref_input` workaround this
panicked; with it, `$1` projects from a separate `Const` node holding the
VALUES instead of from `$0`, and `$2` runs harmlessly but uselessly. With
the guarded program from the summary, the conflict path issues one
statement instead of two, and the workaround and its restriction to the
`Ignore` upsert action are deleted.

The same mechanism generalizes: any pure region whose output is only
consumed under a condition the planner can express is skipped when the
condition fails, and future features that attach loads to maybe-empty row
sources need no per-case correlation workarounds.

## Execution model today

- `Node.deps` is "a superset of the node's data inputs" (`mir/node.rs`):
  value edges and ordering-only edges (e.g. "child INSERT before parent
  INSERT") are one undifferentiated set.
- `LogicalPlan::new` DFS-walks deps from the completion node, producing a
  topologically sorted `execution_order`. Nodes unreachable from completion
  are dropped at plan time; mutations are reachable because their
  consumers hold dep edges on them.
- `exec_plan` runs the action list linearly, each action fully awaited —
  on SQL drivers, inside one transaction when the plan contains more than
  one database operation. Each action stores its result in a refcounted `VarStore`
  slot; `vars.load` hands out the stored response.
- The commit happens after the action loop; the final
  `vars.load(returning)` only reads a slot. Control flow inside the linear
  pass preserves this: everything that runs still runs before commit.

One property to preserve: **effects always run.** A mutation's output may
be unobserved (`exec()` discarding the result) but its database effect is
the point. Guarded regions therefore never contain effectful actions.

## Design

### Edge classification

Split `Node.deps` into:

- `inputs` — value edges: this node reads that node's output. Already
  implicit in each operation's input `NodeId`s; the split makes it
  explicit on the node.
- `after` — ordering-only edges: that node's effect must precede this
  node's, no data flows. (Enclosing-insert edges, sibling-mutation edges.)

The guard analysis needs this split: it reasons about a node's
*consumers*, which are value-edge dependents only. An ordering-only edge
onto a node is not a consumer, and with the undifferentiated `deps` set
the analysis could not tell the two apart and would have to refuse the
guard.

Add `Operation::is_effectful()`: `true` for operations that write —
`Upsert`, `DeleteByKey`, `UpdateByKey`, `ReadModifyWrite`, and
`ExecStatement` when its statement is a mutation **or** its `conditional`
output is not `ConditionalOutput::None`. The second condition matters: the
OCC conditional-write path compiles an UPDATE/DELETE into a
`stmt::Statement::Query` wrapping a data-modifying CTE, so statement kind
alone misclassifies it as a read, and guarding it would skip both the
write and its `condition_failed` error check. Everything else is a read or
pure compute and may be guarded.

The classification serves two invariants: only pure nodes may receive a
guard, and every effectful node must be reachable from the completion node
(`LogicalPlan::new` silently drops unreachable nodes today; add a debug
assertion).

### Guard annotation on MIR nodes

MIR stays a pure DAG; no conditional node type. Statement planning
annotates a pure node with a guard condition when its output is only
consumed under that condition. The first (and initially only) analysis
has two rules:

- a node referenced only inside the body of an `Eval` `map` over node
  `X`'s rows is guarded by `non_empty(X)` — mapping zero rows never
  evaluates the body, so the node's output is unobservable when `X` is
  empty;
- a pure node whose consumers all carry guard `non_empty(X)` inherits
  that guard.

In the motivating plan the load `SELECT` gets the guard from the first
rule (its result feeds only the per-row match expression in the returning
`Eval`), and the back-ref `Project` it reads from gets it from the second.

The analysis is plan-time and conservative — a node with any consumer
outside such a body gets no guard and always executes. This follows
Turso's discipline: the evaluation position of a subquery is static
planner metadata (`phase_floor` in `core/translate/plan.rs`), not a
runtime discovery.

### The `If` action

```rust
enum Cond {
    /// The variable holds a non-empty row list.
    NonEmpty(VarId),
}

Action::If {
    cond: Cond,
    then: Vec<Action>,
    /// Runs when `cond` is false. Assigns every variable the `then` arm
    /// would have produced, so downstream loads never see an unset slot.
    r#else: Vec<Action>,
}
```

Execution planning groups consecutive nodes carrying the same guard into
one `If`, emitted at their existing position in the topological order. The
`else` arm is generated: one `SetVar` per output variable of the `then`
arm, assigning the empty value of the variable's type (`Null` for a
single-row slot, an empty list for a row-list slot). Both arms assign the
same variables, so `VarStore` keeps its current total semantics — every
declared variable is set exactly once per execution, and no consumer needs
an "absent input" concept. (Turso does the same when an outer join misses:
the right side's registers are NULL-filled rather than left unset.)

The interpreter change is small: `exec_step` on an `If` evaluates the
condition against the named variable, then runs one arm's actions
recursively. Testing `NonEmpty` on a stream-backed slot requires a
non-consuming peek: buffer the stream in place (the `Rows::buffer`
pattern pagination already uses) and inspect the first row, leaving the
refcount untouched. A guarded plan therefore materializes its condition
variable's stream. Structured blocks rather than jump opcodes
because Toasty's actions are coarse — a guard wraps one to three actions,
so a block nests at most one level deep, renders as an indented tree in
plan output, and cannot express an invalid jump. The flat-list encoding
can be revisited if plans ever grow turso-like instruction counts.

The else-arm values are placeholders: the guard analysis proved no
expression observes them, and that proof is the soundness argument — the
placeholders are well-typed, so nothing at runtime distinguishes a
correct skip from a wrong guard. A wrongly-guarded node either panics in
expression evaluation (an expression indexing into an empty placeholder
list) or yields a well-typed empty result where data belonged. The
defense is the analysis's conservatism, not a runtime check, which is why
phase 2 scopes it to exactly one pattern.

### Bookkeeping

- `needs_transaction` keeps its static count, including actions inside
  `If` arms. A skipped region can leave a transaction wrapping one
  executed operation; harmless.
- `num_uses` refcounts are computed statically. A skipped `then` arm does
  not consume its inputs, so those slots keep a positive count until plan
  teardown instead of freeing at last use. Accepted: plans are
  short-lived; do not try to keep counts exact under conditional
  execution.
- Debug assertion at exec-plan build: no effectful action inside either
  arm of an `If`.

## Transition plan

Each phase lands independently and keeps the full test suite passing.

1. **Classify.** Split `deps` into `inputs`/`after` at every planner
   site; add `is_effectful()` and the reachability assertion. Purely
   structural.
2. **Guard and emit.** Add the guard annotation, the `map`-body consumer
   analysis, `Action::If`, and the exec-planning grouping. The one
   pattern annotated: the `belongs_to` load subquery attached to an
   insert's returning. The `DO NOTHING` conflict path now skips the
   relation-load `SELECT`; add a regression test asserting via `t.log()`
   that the conflict path issues no `SELECT`.
3. **Delete the workaround.** Remove `back_ref_input` and the
   `Ignore`-only const feed; back-refs read the insert's `RETURNING` rows
   again, which is safe because the reader now only runs when a row
   exists. Net deletion in `plan/statement.rs`.
4. **Widen (separate decisions, as needs arise).** Additional guard
   analyses (other consumption patterns), additional `Cond` variants.
   Nothing further is required for the motivating case.

## Risks

- **Guard analysis soundness.** Guarding a node whose output *is*
  observed when the condition fails substitutes a well-typed placeholder
  for real data — silently, or as a panic when an expression indexes into
  the empty placeholder. No runtime check distinguishes the two cases
  from a correct skip; the only defense is the analysis's conservatism
  (guard only when every value-edge consumer carries the guard). The
  phase-2 scope of exactly one pattern keeps the audit surface small.
- **Implicit ordering not backed by edges.** Guarded nodes still execute
  at their existing position when the condition holds, so ordering only
  changes for skipped executions — and a skipped pure node has no
  observable order. The broader audit of ordering-only edges (needed if
  guards ever move nodes) is deferred with phase 4.

## Alternatives considered

- **Lazy slot forcing** (PostgreSQL's model: a param slot holding an
  unexecuted plan, executed at first read — `ExecEvalParamExec`,
  `src/backend/executor/execExprInterp.c`; the previous revision of this
  design). Behaviorally equivalent for the motivating case, but the
  conditionality is invisible in the plan, demand is discovered at
  runtime inside `Eval` input loading, and forcing triggered by the final
  post-commit read needs a special pre-commit pass. The exec program is
  documented as a virtual machine intended to grow control flow; explicit
  branches fit that architecture, keep the interpreter a linear pass, and
  make plan output show what executes when.
- **Flat jump opcodes** (turso/SQLite VDBE form): `JumpIfEmpty`/`Goto`
  with a program counter. Faithful to the source model, but Toasty's
  actions are coarse enough that label allocation and jump-range
  invariants add nothing over a one-level structured block. Revisit if
  the action set ever becomes fine-grained.
- **Extend the existing `Guard` action** (`exec/guard.rs`): `Guard`
  already evaluates a boolean over inputs, but it gates a *data stream*
  after its producers ran — an empty stream when false. It cannot prevent
  the load `SELECT` from executing, which is the point here. `If`
  subsumes it for execution gating; `Guard` remains the right tool for
  value-level suppression.
- **Per-row correlated rescan** (turso re-emits correlated subquery
  bytecode inside the row loop; PostgreSQL's `ExecScanSubPlan` rescans
  per evaluation): re-executes the subquery per consuming row. In-process
  that is fine; over a network driver it is the N+1 pattern Toasty's
  batch-then-`NestedMerge` design exists to avoid.
- **Keep the const feed.** Works, already shipped, but is a per-case
  correlation workaround restricted to `Ignore` upserts; the next
  maybe-empty row source needs its own. It also fabricates the feed from
  VALUES, which is why it cannot extend to update upserts whose final row
  differs from the VALUES.

## Open questions

- Whether the future parallel interpreter (the architecture doc's stated
  goal of executing independent operations concurrently) schedules an
  `If` block as one unit or its arms' actions individually — deferrable;
  blocks nest the dependency information either way.
- `Cond` vocabulary growth (e.g. a general boolean `eval::Func` like the
  existing `Guard` action uses) — deferrable until a second pattern needs
  it.

## Out of scope

- Loops in the exec program. No current pattern needs iteration; the
  per-row work lives inside `Eval`/`NestedMerge`.
- Skipping effectful actions under any condition. Mutations always run.
- Lazy or re-entrant expression evaluation inside `eval::Func`. The guard
  analysis decides demand at plan time precisely so expression evaluation
  can stay synchronous and strict.
