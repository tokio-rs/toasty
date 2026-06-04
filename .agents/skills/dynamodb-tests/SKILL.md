---
name: dynamodb-tests
description: Always use this skill when running tests against dynamodb (DDB)
---

# Run toasty-driver-integration-suite against DDB and Sqlite use
To run against DDB only use: `cargo  test --features dynamodb  -- --test-threads=1`

# Run test suite against DDB only
Use this when troubleshooting and testing issues specific to DDB: `cargo  test --features dynamodb  --no-default-features -- --test-threads=1`

## Run a specific failing test against DDB

Use this when you need to troubleshoot a specific failing test method: `cargo  test --features dynamodb  --no-default-features <test method name> -- --test-threads=1`