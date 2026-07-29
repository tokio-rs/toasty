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
