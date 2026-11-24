# PostgreSQL password providers

Support a generic asynchronous, fallible password provider when PostgreSQL
establishes a connection. This lets IAM-authenticated Postgres-compatible
databases use short-lived signed tokens without adding a cloud-specific SDK
to Toasty. The [design](../design/postgresql-password-provider.md) defines the
user and driver contract proposed for [#1272].

Implementation follows acceptance of the design in a separate PR. Completion
requires fresh provider invocation on all driver-managed connection paths,
including pool replacement and database reset; unchanged healthy-session
reuse and static setup; bounded creation through existing pool timeouts;
and credential-safe URLs, debug output, errors, and tracing. The design lists
observable acceptance criteria for lifecycle and failure coverage.

[#1272]: https://github.com/tokio-rs/toasty/issues/1272
