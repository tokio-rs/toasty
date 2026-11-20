# PostgreSQL password providers

## Summary

Allow applications to configure an asynchronous, fallible password provider
on `PostgreSQL`. The driver obtains a password when it establishes a new
connection, including replacement connections created by the pool. This
supports short-lived credentials while keeping token signing and credential
refresh policy in application code. Existing URL-based setup stays available.

## Motivation

IAM-authenticated Postgres-compatible databases accept a short-lived signed
token as the password. A token valid for about 15 minutes can expire while a
pool still holds healthy sessions. When the pool later grows or replaces a
closed session, authenticating with the original URL password fails.
[#1272] describes this gap and asks for credentials at connection creation
without a cloud-specific dependency in Toasty.

An application can wrap `Driver::connect` and rebuild the PostgreSQL driver
with a new URL for every call. That workaround must preserve all other
connection settings and encode the token correctly. A password provider lets
applications keep the normal driver and pool without placing tokens in URLs.

[#1272]: https://github.com/tokio-rs/toasty/issues/1272

## User-facing API

### Configure a provider before opening the database

Use a password-free URL for the endpoint, database, user, and TLS settings.
Call `password_provider` before passing the driver to `Db::builder().build`:

```rust
use toasty_driver_postgresql::PostgreSQL;

let driver = PostgreSQL::new(
    "postgresql://app@db.example.com/app?sslmode=verify-full",
)?
.password_provider(|| async { sign_token().await });

let db = toasty::Db::builder()
    .models(toasty::models!(User))
    .build(driver)
    .await?;
```

`sign_token()` is an application function returning `Result<String, E>`.
Sign for the same hostname, port, and username configured in the URL. Keep
cloud SDK configuration and any refresh logic inside that function. Select
verified TLS when the authentication service requires it; the provider does
not enable TLS or relax certificate verification.

The proposed builder signature is:

```rust
pub fn password_provider<F, Fut, E>(self, provider: F) -> Self
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static;
```

Here `Future` is `std::future::Future` and `Result` is `std::result::Result`.
The concrete `PostgreSQL` type remains usable as a `Driver`, including behind
`Box<dyn Driver>`. Callers do not need to name the callback's future type.
Registering a provider is synchronous and does not invoke it. Registering
another provider replaces the previous one.

Capture owned state when signing needs a client or shared configuration:

```rust
let driver = PostgreSQL::new(url)?.password_provider(move || {
    let signer = signer.clone();
    async move { signer.sign_token().await }
});
```

The provider can run concurrently for different connection requests. Use
shared state that supports concurrent access, and avoid blocking the async
runtime. `FnOnce`, borrowed local state, and non-`Send` futures are not accepted.

### Replace one-time token generation

Before, an application embeds a token into a URL at startup:

```rust
let token = sign_token().await?;
let driver = PostgreSQL::new(url_with_password(&token))?;
```

After, it supplies the password when a connection is needed:

```rust
let driver = PostgreSQL::new(password_free_url)?
    .password_provider(|| async { sign_token().await });
```

Static-password users can keep calling `PostgreSQL::new(url)` without a
provider. `Db::builder().connect(url)` remains the static URL convenience API;
use `build(driver)` to configure a provider.

### Configure connection timeouts and retirement separately

A token's validity controls authentication of a new session. It does not
necessarily limit the lifetime of an already authenticated session. Check
your database's session policy instead of assuming that token expiry closes
existing connections.

Use `pool_create_timeout` to bound provider work together with connection
establishment. Without it, the existing default has no creation timeout.
Use `pool_max_connection_lifetime` separately when the service limits session
age. For a service with a 60-minute session limit, a shorter value such as
50 minutes leaves room for connection retirement:

```rust
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    .pool_create_timeout(Some(std::time::Duration::from_secs(10)))
    .pool_max_connection_lifetime(Some(std::time::Duration::from_secs(50 * 60)))
    .build(driver)
    .await?;
```

The lifetime cap is checked when a connection is considered for reuse. It
cannot guarantee that a long query or transaction finishes before a server
session limit; applications must still handle connection loss.

## Behavior

### Connection creation and reuse

The provider is awaited once immediately before each driver-initiated
connection establishment. This includes the initial connection opened by
`build`, pool growth, replacement of expired or broken sessions, and direct
`Driver::connect` calls. Each call obtains its own password; Toasty does not
cache provider results or share a resolved password between connection calls.

Successful provider output replaces both a URL userinfo password and a
`password` query parameter. When a provider is installed, those static
passwords are removed from the driver's retained URL and base configuration.
There is no fallback to either password if the provider fails. The returned
string is passed as a password value to the PostgreSQL client configuration,
not percent-encoded or interpolated into a URL. Other connection settings,
including TLS and startup options, retain their configured values.

Checking out an existing healthy connection, pinging it, executing a query,
or starting a transaction on it does not invoke the provider. Token expiry
does not itself trigger eviction or reauthentication. Any recovery that
creates another connection invokes the provider again, without changing
which statements Toasty may retry.

`reset_db()` also obtains a password separately for every connection it opens,
including its temporary-database sessions. The provider does not grant reset
permissions or make reset available on services that prohibit database DDL.
The lower-level `Connection::connect(config, tls)` API takes caller-supplied
configuration and has no provider; users of that API supply its password.

### Failures, cancellation, and diagnostics

If the provider returns `Err`, no database authentication is attempted and
no connection is added to the pool. A direct driver call returns a Toasty
`driver_operation_failed` error with the fixed message
`PostgreSQL password provider failed`. Arbitrary provider error text and its
source chain are discarded, since either may contain a token or signed URL.
Providers that need detailed diagnostics must arrange their own safe
reporting; never log the token or an unredacted signing request.

Pool acquisition and initial `build` continue to report a `connection_pool`
error around the safe driver error. A creation timeout uses the existing pool
timeout error. Outside the pool, callers can bound direct connection calls
with their own timeout. The provider introduces no automatic retry loop,
including after authentication rejection. A subsequent connection request
invokes the provider again. Existing connections can remain usable when
credential issuance is unavailable, subject to ordinary pool health checks.

For provider-managed connections, authentication and connection-setup
failures preserve their existing Toasty error category but expose fixed,
credential-free messages rather than arbitrary backend text or sources.
Any background connection diagnostic must follow the same rule when it can
carry authentication data. This deliberately trades error detail for safety.
Query errors unrelated to connection setup retain their current behavior.

Dropping a connection-establishment future drops its provider future. Toasty
does not spawn detached credential-refresh tasks. The provider must tolerate
cancellation; any background work it starts remains its responsibility.
Callbacks must report expected failures through `Result`; panics retain the
ordinary Rust behavior and are not converted to authentication errors.

## Edge cases

- An empty successful string is an explicit empty password, with no static
  fallback. A NUL byte is rejected before authentication with a fixed
  `driver_operation_failed` message. Do not truncate or otherwise normalize
  password bytes; punctuation, whitespace, and long signed tokens are valid
  inputs subject to the PostgreSQL client's protocol limits.
- The provider is called per establishment request, not per query or pool
  checkout. Address fallback within one PostgreSQL client establishment
  request may reuse that request's password. A password expiring during slow
  issuance or setup causes an ordinary failure; use a creation timeout and
  return a token with sufficient validity remaining.
- Toasty has no expiry timestamp to inspect. Each invocation must return a
  password valid for the imminent authentication; a provider that caches
  credentials must refresh them before expiry, allowing for setup time and
  clock skew. Callback invocation does not promise a distinct token value.
- The provider changes only the password. It cannot change the endpoint,
  username, database, TLS policy, or signing target. Rebuild the driver when
  those settings change. This proposal does not add multi-host configuration.
- Driver and connection `Debug` output, tracing, errors, and error sources must
  not contain provider output or format captured callback state. Driver debug
  output may indicate that a provider is configured. `Driver::url()` returns
  the password-free configured URL in provider mode, never a generated token.
  PostgreSQL driver `Debug` must redact static URL passwords too.
- Resolved passwords are retained only as needed by the connection attempt
  and underlying client; Toasty must not store them on the driver or cache
  them for later sessions. This API does not promise zeroization of every
  copy held by Rust strings, allocators, or the PostgreSQL client.
- The provider works with the existing TLS feature configuration. Disabling
  TLS does not disable the hook, but it may make the chosen service unusable.
  Other SQL drivers and NoSQL credentials are unaffected.

## Driver integration

This is an additive API on `toasty-driver-postgresql::PostgreSQL`. There are
no new `Driver` or `Connection` methods, capability flags, `ConnectContext`
fields, or `Operation` variants. Out-of-tree drivers need no changes. SQL
serialization and non-SQL encoding contracts are unaffected.

The PostgreSQL driver must apply the provider on every establishment path,
including reset connections, without changing the configured endpoint or
connection options. Provider work stays inside the existing connection
creation timeout. Callback storage must preserve `Driver: Debug + Send +
Sync + 'static` without requiring `Debug` on the callback or its captures.

The implementation PR must demonstrate fresh invocation for initial creation,
pool growth, reconnect/replacement, direct connection, and every reset
connection; no invocation for healthy reuse, ping, or query execution; static
compatibility; and overriding both URL password forms without fallback.
It must also cover concurrent calls, cancellation and timeouts, provider and
authentication failures, exact password bytes, and token absence from driver
and connection `Debug`, `Driver::url`, tracing, error display, and error
sources. Include a failure whose provider error contains a sentinel secret.
Use a controlled credential issuer and connection fixture to show that a new
session works after the startup token has expired, without waiting 15 minutes.

## Alternatives considered

**Static token in the URL.** This leaves later connection attempts using an
expired token. Retiring sessions sooner cannot refresh that fixed password.

**Application Driver wrapper.** A wrapper can obtain a token per `connect`,
but must preserve configuration, password encoding, and reset behavior. Keep
it as a workaround rather than the normal way to use rotating credentials.

**A provider for the entire connection configuration.** This could rotate
users and endpoints too, but makes TLS and destination changes part of every
connection attempt. The motivating issue only needs a password.

**A shared provider on the Driver trait or ConnectContext.** That would impose
an authentication abstraction on drivers with different session or request
lifecycles. A PostgreSQL-specific builder keeps this change additive.

**Built-in IAM support.** A cloud-specific SDK and signing policy would couple
Toasty to one issuer. A generic asynchronous callback supports other issuers
and lets applications choose their SDK and refresh behavior.

**Forward provider errors unchanged.** Preserving arbitrary error chains is
convenient for debugging, but the pool displays those chains in tracing and
returned errors. Fixed messages make the credential boundary explicit.

## Open questions

- **Builder naming (blocking acceptance).** `password_provider` is proposed;
  maintainers may prefer `with_password_provider` or `with_password_fn` to
  align with the Turso callback naming. The lifecycle contract is unchanged.
- **Safe diagnostic detail (deferrable).** Fixed messages are the default.
  A richer error API needs a separate, explicit credential-safe contract.

## Out of scope

- Cloud SDKs, token signing, and credential caches: owned by the application.
- Live reauthentication or password updates on existing sessions: controlled
  by the database protocol and server, not this connection factory hook.
- New pool retirement, retry, or transaction semantics: existing controls
  remain separate from password generation.
- Rotating endpoints, users, databases, or client certificates: requires a
  broader connection-configuration design.
- A cross-driver authentication framework: request-based and session-based
  backends have different credential lifecycles.
- Guaranteed memory zeroization: requires support throughout the client stack.
- Changes to standalone URL parsing errors before provider registration:
  use a password-free URL rather than embedding a signed token at construction.
