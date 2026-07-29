# Test Suite Minimization

Test suite minimization removes test executions and generated test code that
exercise behavior already covered elsewhere. A change is acceptable only when
it preserves the suite's observable checks and covered product-code regions.

## Measure the Driver Suite

Use the same feature selection as the driver CI job. The SQLite job requires no
external service:

```bash
/usr/bin/time -lp env \
  CARGO_TARGET_DIR=/tmp/toasty-test-target \
  CARGO_BUILD_JOBS=1 \
  cargo test --no-default-features --features sqlite --no-run

/tmp/toasty-test-target/debug/deps/sqlite-<hash> --list --format terse
/tmp/toasty-test-target/debug/deps/sqlite-<hash> --quiet
```

Use a new target directory for each cold-build comparison. Record wall time,
maximum resident memory, target-directory size, integration-test binary size,
test count, and execution time.

## Capture Product Coverage

Install `cargo-llvm-cov` and the toolchain's `llvm-tools-preview` component.
Then capture a baseline:

```bash
scripts/capture-driver-coverage.sh sqlite /tmp/sqlite-baseline.json
```

The script instruments the selected driver integration binary. It excludes
test bodies, examples, and integration-suite scaffolding from the report so
removing a test does not count the removed test source as lost product
coverage.

MySQL, PostgreSQL, DynamoDB, and Turso use the same command with their feature
name. Start the service and set the environment variables required by the
matching CI job before capturing those reports.

## Compare a Candidate

Capture another report after pruning, then compare the exact covered regions:

```bash
scripts/capture-driver-coverage.sh sqlite /tmp/sqlite-candidate.json
scripts/compare-test-coverage.sh \
  /tmp/sqlite-baseline.json \
  /tmp/sqlite-candidate.json
```

The comparison normalizes monomorphized functions by source location. Multiple
generic instantiations of the same source region therefore count once. The
candidate may execute a region fewer times, because removing duplicate
executions is the purpose of the change. A region that changes from covered to
uncovered fails the comparison.

Pass multiple candidate reports to compare their union against the baseline.
This supports test shards and per-driver reports without merging LLVM profile
data:

```bash
scripts/compare-test-coverage.sh \
  /tmp/baseline.json \
  /tmp/candidate-part-1.json \
  /tmp/candidate-part-2.json
```

Coverage is one acceptance gate, not the complete argument for removal. Each
pruning change must also:

1. Identify the behavior represented by the removed case.
2. Name the retained test or cases that cover that behavior.
3. Preserve distinct input domains, error outcomes, database capabilities, and
   generated API forms.
4. Run the affected test targets on the pinned stable toolchain.
5. Run `cargo fmt` and `cargo clippy`.

For type matrices, retain focused cases for every supported representation.
Do not repeat the full behavioral matrix for each representation when those
cases execute the same product regions.

## Reference Minimization

The July 2026 minimization used SQLite as the service-free runtime and coverage
reference. Cold builds used a new target directory, one Cargo job, and the same
workspace command before and after the change.

| Metric | Before | After | Reduction |
|---|---:|---:|---:|
| Cold compile wall time | 345.11 s | 259.52 s | 24.8% |
| SQLite driver tests | 1,354 | 763 | 43.6% |
| SQLite driver execution | 0.68 s | 0.39 s | 42.6% |
| SQLite integration binary | 166,749,544 bytes | 115,636,392 bytes | 30.6% |
| Cold target directory | 8.5 GiB | 6.4 GiB | about 25% |

The exact-region baseline contained 30,392 covered product regions. The final
candidate covered 30,398 regions and missed none of the baseline regions. The
candidate is therefore a strict coverage superset under the normalization
described above.

The reduction came from four classes of waste:

1. Running the full behavior matrix for both UUID and integer IDs. The suite
   now keeps UUID as the default and focused integer representatives for
   type-specific behavior.
2. Separate tests that repeated a local model and database setup for compatible
   cases. These cases now use independent rows in one fixture.
3. Separate wrappers around query-macro forms that used the same scenario,
   capability, and compatible dataset.
4. Empty relation cases that performed no operation or assertion.

The convergence audit stopped at repetitions that preserve a practical
distinction:

- different database capabilities or native operators;
- different scalar, document, smart-pointer, or composite-key domains;
- different relationship topologies;
- transaction, rollback, stale-write, and failure-state isolation;
- generated API forms with distinct type-checking behavior.

Shared scenarios are defined once at module scope. Reusing the same scenario in
several `#[driver_test]` functions does not regenerate its models, so an equal
scenario attribute alone is not evidence of compile-time duplication.

Final validation included the complete SQLite workspace test run, UI tests,
doctests, an all-targets Clippy pass, a clean affected-crate Clippy rerun after
fixing its warning, a PostgreSQL no-run integration build, and the exact-region
comparison. Service-backed runtime coverage for PostgreSQL, MySQL, DynamoDB,
and Turso still requires their CI services and should use the same
capture-and-compare procedure before backend-specific pruning.
