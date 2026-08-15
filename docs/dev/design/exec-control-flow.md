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
    Release($0)      // the load Project would have performed
    $2 = SetVar([])
$3 = Eval(map $0 rows -> Post record, user slot from $2)
return $3
```

The `Release` in the else arm is part of a second, self-standing fix the
`If` machinery motivates: variable refcounts become exact (counted from
value edges, with unconsumed outputs never stored), so slots free at
last use instead of lingering until plan teardown — see "Exact variable
lifetimes".

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

### Value edges and ordering edges

The two edge kinds already exist structurally; they only lack names:

- **Value edges** are the operations' own input fields (`Eval.inputs`,
  `Project.input`, `ExecStatement.inputs`, …) — the `NodeId`s `to_exec`
  wires into variable slots.
- **`Node.deps`** is seeded from those inputs (`From<Operation> for
  Node`, `mir/operation.rs`) and then extended by planner sites with
  ordering-only edges (enclosing-insert edges, sibling-mutation edges).
  It is the scheduling relation: `LogicalPlan::new`'s topological sort
  and `num_uses` counting walk it, and it keeps that job unchanged.

The guard analysis reasons about a node's *consumers* — nodes that read
its output. Consumers are the reverse of the value edges, so the
analysis reads operation inputs, not `deps`; an ordering-only dep never
looks like a consumer. Factor the input enumeration out of the `From`
impl into an `Operation::inputs()` accessor so the analysis and node
construction share it. No per-site migration of `deps` is needed; if the
ordering-only edges are ever wanted as a set (assertions, a future
ordering audit), they are derivable as `deps − inputs()`.

Add `mir::Operation::is_effectful()` (distinct from the existing
`exec::Action::is_db_op()`, which answers "issues a driver operation" for
transaction wrapping — queries are db ops but not effectful): `true` for
operations that write —
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

MIR stays a pure DAG; no conditional node type. A pure node is annotated
with a guard condition when its output is only consumed under that
condition. The annotation is a pass over the finished graph, run at
`LogicalPlan::new` alongside the reachability assertion — not
incrementally during statement planning, where a node's consumer set is
still growing (the back-ref `Project` exists before the child statement
that reads it) and a consumer created after an annotation could
invalidate it. The first (and initially only) analysis has two rules:

- a node referenced only inside the body of an `Eval` `map` over node
  `X`'s rows is guarded by `non_empty(X)` — mapping zero rows never
  evaluates the body, so the node's output is unobservable when `X` is
  empty. Which input positions are map-body-only is read from the
  `Eval`'s stored function;
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
    /// Runs when `cond` is false. Generated: placeholder assignments for
    /// the `then` arm's escaping outputs and releases for its external
    /// input loads (see below).
    r#else: Vec<Action>,
}
```

Execution planning groups each maximal run of consecutive same-guard
nodes in the topological order into one `If`, emitted at the run's
existing position. Nothing guarantees all same-guard nodes are adjacent —
an unguarded chain can be interleaved between two guarded ones — in which
case one guard produces several `If` blocks with the same condition. That
is sound: the guard rules guarantee an interleaved unguarded node never
consumes a guarded output, and the variable classification below is per
block. The `else` arm is generated from a static classification of the
variables the `then` arm touches:

- **Escaping outputs** (produced inside, consumed outside the block): one
  `SetVar` each, assigning the empty value of the variable's type (`Null`
  for a single-row slot, an empty list for a row-list slot), with the
  variable's external use count. Consumers outside the block therefore
  never see an unset slot. (Turso does the same when an outer join
  misses: the right side's registers are NULL-filled rather than left
  unset.)
- **External inputs** (produced outside, loaded inside): one release per
  load the `then` arm would have performed, so `num_uses` refcounts stay
  exact on both paths. A release decrements the slot's count and drops
  the entry at zero — unlike `load`, no stream duplication is needed.
- **Internal variables** (produced and consumed inside): untouched. They
  are never observed outside the block; on the else path their slots are
  never created.

The interpreter change is small: `exec_step` on an `If` evaluates the
condition against the named variable, then runs one arm's actions
recursively (`exec_step` is async, so the recursion is boxed — or, since
arms never nest today, a leaf dispatch). Testing `NonEmpty` on a stream-backed slot requires a
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
phase 3 scopes it to exactly one pattern.

### Exact variable lifetimes

The `If` else-arm releases only keep counts exact if they were exact to
begin with — and they are not. `num_uses` is incremented once per `deps`
edge (`compute_operation_execution_order`), but only value-edge
consumers ever call `vars.load`, so every ordering-only edge inflates a
slot's count with no draining load: a const-returning insert's
`ExecStatement` response stays pinned in the store until teardown, and
the existing `Guard` action's false path never loads its suppressed
`input` — potentially the largest value in the plan. Since the `If` work
introduces the release machinery anyway, this design fixes variable
lifetimes outright rather than layering exact arms on inexact counts:

- **Count from value edges, per load.** `num_uses` becomes the number of
  loads its consumers perform, plus one exit use for the final
  `vars.load(returning)` (always present — every node registers a
  variable, so `plan.returning` is never `None`). For most actions the
  load count equals the deduplicated input set, but the counting rule is
  loads, not edges: `Guard` loads `input` in addition to every
  `guard_inputs` entry, so a variable appearing in both counts twice
  even though the deps set holds it once. An `Operation::input_loads()`
  iterator with multiplicity (rather than the deduplicated `inputs()`)
  feeds the count. Ordering-only edges keep scheduling and stop
  counting.
- **Zero-use outputs are not stored.** A mutation whose response nobody
  reads now has `num_uses == 0`; its action skips `vars.store` (the
  output variable becomes optional). Driver responses for discarded
  results no longer occupy slots at all.
- **Conditional consumers release.** The invariant: every declared input
  is either loaded or released exactly once per execution. `If` else
  arms satisfy it by construction; `Guard`'s false path gains a release
  of its `input`, fixing the existing leak.
- **Teardown assertion.** With counts exact, "every slot is empty" holds
  after the final `vars.load(returning)` and is debug-asserted there —
  on the success path only. A mid-plan failure returns after rollback
  with remaining loads legitimately unperformed; the assertion must not
  run on that path. The failure directions are asymmetric: undercounting
  panics loudly on a load of a freed slot; overcounting leaks silently —
  the assertion converts the silent direction into a loud one.
- **Per-action audit.** One pass over the exec actions confirming each
  action's loads match its declared `input_loads()` or release on every
  declining path. Most are trivially "loads each once upfront" (`Eval`,
  `Project`, `Filter`, `NestedMerge`). Known exceptions: `Guard`
  (conditional `input` load, two overlapping input fields) and
  `ReadModifyWrite`, which declares `inputs` but asserts them empty at
  exec — record it as declares-but-asserts-empty so a future non-empty
  RMW input doesn't silently violate the counting.

### Bookkeeping

- `needs_transaction` keeps its static count, including actions inside
  `If` arms. A skipped region can leave a transaction wrapping one
  executed operation; harmless.
- Debug assertion at exec-plan build: no effectful action inside either
  arm of an `If`.

## Transition plan

Each phase lands independently and keeps the full test suite passing.

1. **Classify.** Factor `Operation::inputs()` out of the `From` impl;
   add `is_effectful()` and the reachability assertion. Purely
   structural; `Node.deps` is untouched.
2. **Exact refcounts.** Count `num_uses` from per-load value edges
   (`Operation::input_loads()`) plus the exit use; add
   `VarStore::release`; skip storing zero-use outputs; release `Guard`'s
   suppressed input on its false path; run the per-action load audit;
   add the success-path teardown assertion. Lands independently of
   control flow and fixes existing leaks on its own.
3. **Guard and emit.** Add the guard annotation, the `map`-body consumer
   analysis, `Action::If`, the non-consuming peek accessor on
   `VarStore`, and the exec-planning grouping. The one
   pattern annotated: the `belongs_to` load subquery attached to an
   insert's returning. The `DO NOTHING` conflict path now skips the
   relation-load `SELECT`; add a regression test asserting via `t.log()`
   that the conflict path issues no `SELECT`.
4. **Delete the workaround.** Remove `back_ref_input` and the
   `Ignore`-only const feed; back-refs read the insert's `RETURNING` rows
   again, which is safe because the reader now only runs when a row
   exists. Net deletion in `plan/statement.rs`.
5. **Widen (separate decisions, as needs arise).** Additional guard
   analyses (other consumption patterns), additional `Cond` variants.
   Nothing further is required for the motivating case.

## Risks

- **Guard analysis soundness.** Guarding a node whose output *is*
  observed when the condition fails substitutes a well-typed placeholder
  for real data — silently, or as a panic when an expression indexes into
  the empty placeholder. No runtime check distinguishes the two cases
  from a correct skip; the only defense is the analysis's conservatism —
  guard only when every reference to the node's output is proven
  unobservable under the failed condition (rule 1's map-body membership,
  rule 2's guarded-consumer closure). The phase-3 scope of exactly one
  pattern keeps the audit surface small.
- **Implicit ordering not backed by edges.** Guarded nodes still execute
  at their existing position when the condition holds, so ordering only
  changes for skipped executions — and a skipped pure node has no
  observable order. A broader audit of ordering-only edges becomes
  necessary only if a future change moves guarded nodes from their
  topological position; nothing in this design does.

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
