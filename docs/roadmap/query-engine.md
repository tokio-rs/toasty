# Query Engine Optimization Roadmap

## Overview

The query engine currently performs simplification as a single `VisitMut` pass that
applies local rewrite rules bottom-up. This works well for straightforward
transformations (constant folding, tuple decomposition, association rewriting),
but it has structural limitations as the optimizer takes on more complex work.

This document tracks improvements to the query engine's optimization
infrastructure, focusing on predicate simplification and the compilation
pipeline.

## Current State

### Simplification Pass

The simplifier (`engine/simplify.rs`) implements `VisitMut` and applies rules in
a single bottom-up traversal. Each node is visited once, simplified, and then
its parent is simplified with the updated children.

**What works well:**
- Local rewrites: constant folding, boolean identity, tuple decomposition
- Association rewriting and subquery lifting
- Match elimination (distributing binary ops over match arms)

**Structural limitations:**
- Rules fire during the walk, so ordering matters. A rule that produces
  expressions consumable by another rule only works if the consumer fires later
  in the same walk or the walk is re-run.
- Global analysis (e.g., detecting contradictions across an entire AND
  conjunction) must be done inline during the walk, mixing local and global
  concerns.
- Expensive analyses run on every AND node encountered, even when only a small
  fraction would benefit.

### Contradicting Equality Detection

The simplifier currently detects `a = c1 AND a = c2` (where c1 != c2) inline in
`simplify_expr_and`. This is O(n^2) in the number of equality predicates within a
single AND. While operand lists are typically small, the analysis runs on *every*
AND node during the walk, including intermediate nodes that are about to be
restructured by other rules.

## Planned Improvements

### Phase 1: Post-Lowering Optimization Pass

Move expensive predicate analysis out of the per-node simplifier and into a
dedicated pass that runs once after lowering, against the HIR representation.
At this point the statement is fully resolved to table-level expressions and the
predicate tree is stable — no more association rewrites or field resolution
changes will restructure it.

This pass would handle:
- Contradicting equality pruning
- Redundant predicate elimination
- Tautology detection

**Why after lowering:** Before lowering, predicates reference model-level fields
and contain relationship navigation that the lowering phase rewrites. Running
global analysis before this rewriting is wasted work — the predicate tree will
change. After lowering, the predicates are in their final structural form (column
references, subqueries), so analysis results are stable.

### Phase 2: Equivalence Classes

Build equivalence classes from equality predicates before running constraint
analysis. When the optimizer sees `a = b AND b = c`, it should know that `a`,
`b`, and `c` are all equivalent, enabling:

- **Transitive contradiction detection**: `a = b AND b = 5 AND a = 7` is a
  contradiction (a must be both 5 and 7), even though no single pair of
  predicates directly conflicts.
- **Predicate implication**: `a = 5 AND a > 3` — the second predicate is
  implied and can be dropped.
- **Join predicate inference**: If `a = b` and a filter constrains `a`, the
  same constraint applies to `b`.

Equivalence classes are a standard technique in query optimizers. The idea is to
union-find expressions that are constrained to be equal, then check each class
for conflicting constant bindings or range constraints.

### Phase 3: Structured Constraint Analysis

Replace ad-hoc pairwise comparisons with a more structured representation of
constraints. For each expression (or equivalence class), maintain:

- **Constant binding**: The expression must equal a specific value
- **Range bounds**: Upper/lower bounds from inequality predicates
- **NOT-equal set**: Values the expression cannot be (from `!=` predicates)

With this structure, contradiction detection becomes a property check rather than
a search: an expression with two different constant bindings, or a constant
binding outside its range bounds, is immediately contradictory.

### Predicate Normalization (Not Full DNF)

Full conversion to disjunctive normal form (DNF) — where the entire predicate
becomes an OR of ANDs — risks exponential blowup. A predicate with N
AND-connected clauses of M OR-options each expands to M^N terms. This makes
full DNF impractical as a general-purpose transformation.

Instead, apply targeted normalization:

- **Flatten associative operators**: Merge nested `AND(AND(...), ...)` and
  `OR(OR(...), ...)` into flat lists (already done).
- **Canonicalize comparison direction**: Ensure constants are on the right side
  of comparisons (already done).
- **Limited distribution**: Distribute AND over OR only in specific cases where
  it enables index utilization or constraint extraction, with a size budget to
  prevent blowup.
- **OR-of-equalities to IN-list**: Convert `a = 1 OR a = 2 OR a = 3` to
  `a IN (1, 2, 3)` for more efficient execution.

The goal is to normalize enough for the constraint analysis to work without
paying the exponential cost of full DNF.

## Design Principles

- **Run expensive analysis once, not per-node.** The current simplifier
  intermixes cheap local rewrites with expensive global analysis. Separate them.
- **Analyze after the predicate tree is stable.** Post-lowering is the right
  point — predicates are resolved to columns and won't be restructured.
- **Build structure, then query it.** Constructing equivalence classes and
  constraint summaries up front makes individual checks cheap.
- **Budget-limited transformations.** Any rewrite that can expand expression
  size (distribution, case expansion) must have a size limit.
