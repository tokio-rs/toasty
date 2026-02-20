# Upsert Support

## What Upsert Is

Upsert is an atomic insert-or-update: if no row exists matching a conflict target (primary key or unique constraint), insert; if one does, either update it or leave it alone. All major databases provide a native atomic form:

| Database | Syntax |
|---|---|
| PostgreSQL / SQLite | `INSERT ... ON CONFLICT (cols) DO UPDATE SET ...` / `DO NOTHING` |
| MySQL / MariaDB | `INSERT ... ON DUPLICATE KEY UPDATE ...` |
| SQL Server / Oracle | `MERGE INTO ...` |

The atomic nature matters — the SELECT + INSERT/UPDATE approach has a race condition window under concurrent load.

## Key Design Dimensions

### 1. Semantics

- **Insert-or-update**: insert if absent, update specified columns if conflicting.
- **Insert-or-ignore** (no-op on conflict): `ON CONFLICT DO NOTHING` / `INSERT IGNORE`.

### 2. Conflict Target

- **PostgreSQL / SQLite**: explicit — name the columns or constraint (`ON CONFLICT (col)`, `ON CONFLICT ON CONSTRAINT name`). Supports partial indexes (`ON CONFLICT (col) WHERE predicate`).
- **MySQL**: implicit — fires on any matching PRIMARY KEY or UNIQUE index. Cannot disambiguate when multiple unique constraints exist.
- **MERGE (SQL Server / Oracle)**: a join predicate; more expressive but verbose.

### 3. Column Update Control

On conflict, ORMs generally let you:
- Update all non-key columns (default in most).
- Update a named subset (`update_only` in ActiveRecord, `set` in Drizzle/TypeORM).
- Reference the proposed (rejected) row via the `EXCLUDED` pseudo-table (PostgreSQL/SQLite) or `VALUES(col)` / row alias (MySQL ≥ 8.0.19).
- Conditional update: add a `WHERE` to the `DO UPDATE` clause to skip the write when nothing changed (`IS DISTINCT FROM` trick).

### 4. Bulk vs. Single-Row

Native SQL upserts are inherently multi-row (multi-value `INSERT ... VALUES (...), (...), ...`). Most ORMs expose both a single-record API and a bulk API; they map to the same statement.

### 5. Lifecycle / Callbacks

All ORMs bypass model callbacks, validations, and lifecycle hooks in bulk upsert paths. This is a deliberate performance trade-off. Single-record upserts (e.g., Prisma's `upsert()` legacy path) may trigger them.

## How Major ORMs Handle Upserts

### Drizzle ORM

API is deliberately close to SQL. PostgreSQL/SQLite use `.onConflictDoUpdate({ target, set })` and `.onConflictDoNothing()`. MySQL uses `.onDuplicateKeyUpdate({ set })` — no conflict target since MySQL's syntax doesn't support one. Partial index targets via `targetWhere`; conditional update via `setWhere`. No cross-DB abstraction — separate methods per dialect family.

```ts
// PostgreSQL / SQLite
db.insert(users).values({ id: 1, name: 'John' })
  .onConflictDoUpdate({ target: users.id, set: { name: sql`excluded.name` } });

// MySQL
db.insert(users).values({ id: 1, name: 'John' })
  .onDuplicateKeyUpdate({ set: { name: sql`values(${users.name})` } });
```

No SQL Server support for upsert. No "update all non-key columns" shorthand — you enumerate `set` explicitly.

- [Drizzle upsert guide](https://orm.drizzle.team/docs/guides/upsert)
- [Drizzle insert reference](https://orm.drizzle.team/docs/insert)

### Prisma

High-level record-at-a-time `upsert({ where, create, update })`. The `where` must reference exactly one unique field. Since v4.6.0, Prisma uses native `INSERT ... ON CONFLICT` ("native upsert") when: no nested relations in `create`/`update`, only one model touched, one unique field in `where`, and that field has the same value in `create`. Everything else falls back to SELECT + INSERT/UPDATE — which has a race condition and can produce `P2002` errors under concurrency.

No `upsertMany`. Bulk insert-or-ignore only via `createMany({ skipDuplicates: true })`, which generates `ON CONFLICT DO NOTHING` (PostgreSQL) or `INSERT IGNORE` (MySQL); not supported on SQLite or SQL Server.

No access to `EXCLUDED` pseudo-table; atomic operations (`{ increment: 1 }`) are the only way to express relative updates.

- [Prisma client reference](https://www.prisma.io/docs/orm/reference/prisma-client-reference)
- [GitHub #9972: upsert should use ON CONFLICT](https://github.com/prisma/prisma/issues/9972)
- [GitHub #18883: compound PK upsert](https://github.com/prisma/prisma/issues/18883)

### Hibernate (6.3+ / 6.5+)

Two APIs:

**`StatelessSession.upsert(entity)`** (6.3+): entity-level, bypasses first-level cache. Translates to `MERGE` on PostgreSQL/Oracle/SQL Server. On MySQL, issues an UPDATE then conditional INSERT (two round trips — not a single atomic statement).

**JPQL `INSERT ... ON CONFLICT DO UPDATE SET ...`** (6.5+): the most expressive. A JPQL extension that Hibernate translates per-dialect: PostgreSQL gets `ON CONFLICT`, MySQL gets `ON DUPLICATE KEY UPDATE`, Oracle/SQL Server get `MERGE`. `EXCLUDED` pseudo-table is available in the JPQL syntax and translates correctly per dialect. `ON CONFLICT DO NOTHING` also supported.

```java
entityManager.createQuery("""
    insert into Book (id, title, isbn) values (:id, :title, :isbn)
    on conflict(id) do update set title = excluded.title, isbn = excluded.isbn
    """).executeUpdate();
```

Spring Data JPA's HQL parser doesn't recognize the `ON CONFLICT` extension — use native queries there.

- [Hibernate ON CONFLICT DO clause — Vlad Mihalcea](https://vladmihalcea.com/hibernate-on-conflict-do-clause/)
- [Hibernate StatelessSession upsert](https://vladmihalcea.com/hibernate-statelesssession-upsert/)
- [Baeldung: ON CONFLICT for Hibernate](https://www.baeldung.com/hibernate-insert-query-on-conflict-clause)

### TypeORM

`repository.upsert(data, conflictPaths)` or QueryBuilder's `.orUpdate(updateCols, conflictCols)`. By default updates all provided non-key columns on conflict. `skipUpdateIfNoValuesChanged: true` adds a `WHERE ... IS DISTINCT FROM ...` guard (PostgreSQL only). `@UpdateDateColumn` and `@VersionColumn` are automatically managed in the generated SET clause.

PostgreSQL/SQLite use `ON CONFLICT`, MySQL uses `ON DUPLICATE KEY UPDATE`, Oracle/SQL Server/SAP HANA use `MERGE`. Cannot target a constraint by name (column list only). Relation fields in `conflictPaths` can behave unexpectedly — use raw column names.

- [TypeORM Repository API](https://typeorm.io/docs/working-with-entity-manager/repository-api/)
- [upsert Oracle/SQL Server support commit](https://github.com/typeorm/typeorm/commit/a9c16ee66d12d327e2ad9a511c8223bb72d4e693)

### SQLAlchemy

No cross-database abstraction. Dialect-specific `insert()` constructors:

- `sqlalchemy.dialects.postgresql.insert` → `.on_conflict_do_update(index_elements, set_, where)` / `.on_conflict_do_nothing()`
- `sqlalchemy.dialects.sqlite.insert` → same interface
- `sqlalchemy.dialects.mysql.insert` → `.on_duplicate_key_update(...)` using `.inserted` (not `.excluded`) to reference the proposed row

`Session.merge()` provides ORM-level insert-or-update by primary key (SELECT + INSERT/UPDATE, cross-database, subject to race condition). Python-side `Column(onupdate=...)` hooks do NOT fire during dialect-level upserts — must be included explicitly in `set_`.

- [SQLAlchemy ORM DML guide](https://docs.sqlalchemy.org/en/20/orm/queryguide/dml.html)
- [SQLAlchemy PostgreSQL dialect](https://docs.sqlalchemy.org/en/20/dialects/postgresql.html)
- [SQLAlchemy MySQL dialect](https://docs.sqlalchemy.org/en/20/dialects/mysql.html)

### ActiveRecord (Rails 6+)

`upsert` (single) and `upsert_all` (bulk). Bypasses callbacks and validations entirely. On conflict, updates all non-key, non-readonly columns by default. Rails 7+ adds:
- `update_only: [:col1, :col2]` — restrict to a subset
- `on_duplicate: Arel.sql("price = LEAST(books.price, EXCLUDED.price)")` — raw SQL for SET clause
- `record_timestamps: false` — skip auto-timestamps
- `returning: %w[id title]` — PostgreSQL/SQLite/MariaDB only

`unique_by` (conflict target by column or index name) is PostgreSQL/SQLite only; MySQL ignores it. `created_at` is preserved on conflict; `updated_at` is updated. `insert_all` (insert-or-ignore) maps to `ON CONFLICT DO NOTHING` or `INSERT IGNORE`.

- [Rails API: upsert_all](https://api.rubyonrails.org/classes/ActiveRecord/Persistence/ClassMethods.html)
- [Rails 7 upsert_all new options](https://blog.kiprosh.com/rails-7-adds-new-options-to-upsert_all/)

## Cross-ORM Comparison

| Feature | Drizzle | Prisma | Hibernate 6.5 | TypeORM | SQLAlchemy | ActiveRecord |
|---|---|---|---|---|---|---|
| Insert-or-update | Yes | Yes | Yes | Yes | Yes | Yes |
| Insert-or-ignore | Yes | `skipDuplicates` | Yes | Yes | Yes | `insert_all` |
| Bulk upsert | Yes | No | No (loop) | Yes | Yes | `upsert_all` |
| Explicit conflict target | Yes (PG/SQLite) | Implicit | Yes | Yes | Yes | Yes (PG/SQLite) |
| Conflict target by constraint name | No | No | No | No | Yes | Yes |
| Partial index target | Yes | No | No | Yes (PG) | Yes (PG) | No |
| Conditional update (`WHERE`) | Yes | No | No | `skipUpdateIfNoValuesChanged` | Yes (PG) | Via raw SQL |
| Access proposed row (`EXCLUDED`) | Via raw SQL | No | Yes (JPQL) | Auto | `.excluded` / `.inserted` | Via `Arel.sql` |
| Update column subset | Enumerate `set` | `update` object | Enumerate `SET` | Enumerate cols | Enumerate `set_` | `update_only:` |
| Cross-DB portable API | No | Yes (limited) | Yes (JPQL) | Mostly | No | Mostly |
| Bypasses callbacks | N/A | Yes (native path) | Yes | Yes | Yes | Yes |

## Observations Relevant to Toasty

- **MySQL's implicit conflict target** is a fundamental difference from PostgreSQL/SQLite — any cross-DB upsert abstraction must account for this. Most ORMs expose different method names rather than papering over the difference.
- **The `EXCLUDED` pseudo-table** (or `VALUES(col)` on MySQL) is the key ergonomic gap between "update to a constant" and "update to the proposed value." Hibernate's JPQL layer is the only one that abstracts this uniformly.
- **Bulk upsert is the common case** in practice (data sync, seeding, ETL). Prisma's lack of `upsertMany` is a frequently cited limitation. The SQL shape is the same as single-row; the driver API difference is just multi-row `VALUES`.
- **Lifecycle bypass** is universally expected. There is no ORM that runs callbacks during a bulk `INSERT ... ON CONFLICT`.
- **SQL Server / Oracle** require `MERGE`, which is structurally different from `INSERT ... ON CONFLICT`. Most ORMs either skip these databases for upsert or generate `MERGE` behind the scenes. Toasty currently has no SQL Server / Oracle drivers, so this is not an immediate concern.
- **DynamoDB**: uses `PutItem` (unconditional replace) or `ConditionExpression: "attribute_not_exists(pk)"` for insert-only. There is no native "update specific fields if exists" — that requires `UpdateItem` with a separate existence check or a transaction.
