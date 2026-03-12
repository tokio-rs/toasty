# Gaps: Polymorphism, Deferred Fields, Upsert, Raw SQL, and DynamoDB Migrations

This guide summarizes major unimplemented data-model and query capabilities,
plus the current DynamoDB migration limitation.

26. Polymorphic associations (not implemented)
27. Deferred field loading (`#[deferred]`, `Deferred<T>`) (not implemented)
28. Upsert API (not implemented)
29. Raw SQL escape hatch (not implemented)
30. DynamoDB migrations (not implemented)

## 26) Polymorphic Associations (Not Implemented)

There is no built-in polymorphic relation API yet (for example, one model
belonging to either `Post` or `Photo` through a single typed relation field).

Use explicit join tables per target type today.

```rust
#[derive(Debug, toasty::Model)]
struct PostLike {
    #[key]
    #[auto]
    id: u64,

    #[index]
    post_id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = post_id, references = id)]
    post: toasty::BelongsTo<Post>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

#[derive(Debug, toasty::Model)]
struct CommentLike {
    #[key]
    #[auto]
    id: u64,

    #[index]
    comment_id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = comment_id, references = id)]
    comment: toasty::BelongsTo<Comment>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}
```

This keeps relations explicit and works with current typed relation APIs.

## 27) Deferred Field Loading (Not Implemented)

`#[deferred]` and `Deferred<T>` are planned but not implemented.

Split large columns into a separate model/table and load on demand.

```rust
#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,
    title: String,
}

#[derive(Debug, toasty::Model)]
struct ArticleBody {
    #[key]
    article_id: u64,

    #[belongs_to(key = article_id, references = id)]
    article: toasty::BelongsTo<Article>,

    body: String,
}
```

```rust
let articles = Article::all().collect::<Vec<_>>(&mut db).await?;
let body = ArticleBody::get_by_article_id(&mut db, &articles[0].id).await?;
```

## 28) Upsert API (Not Implemented)

There is no first-class Toasty upsert API yet.

Use a transaction + lookup + update-or-create flow.

```rust
let mut tx = db.transaction().await?;

match User::filter_by_email(email).first(&mut tx).await? {
    Some(mut user) => {
        user.update().name(name).exec(&mut tx).await?;
    }
    None => {
        User::create()
            .email(email)
            .name(name)
            .exec(&mut tx)
            .await?;
    }
}

tx.commit().await?;
```

For concurrent writers, keep a unique constraint on the conflict key and handle
retry on unique-violation errors.

## 29) Raw SQL Escape Hatch (Not Implemented)

There is currently no official Toasty API for embedding raw SQL fragments in
typed queries.

Use this split approach:

- Use Toasty query/update builders for expressible operations.
- For unsupported query shapes, call your backend client directly for that
  specific operation.

Example (outside Toasty):

```rust
sqlx::query("UPDATE users SET score = score + 1 WHERE id = ?")
    .bind(user_id)
    .execute(&pool)
    .await?;
```

## 30) DynamoDB Migrations (Not Implemented)

Migration generation/apply tracking is not implemented for the DynamoDB driver.

Use this operational approach:

- Use `db.push_schema().await?` for initial table/index creation in controlled
  environments.
- Apply schema evolution manually via AWS tooling (SDK/CLI/console) for
  production changes.

```rust
let mut db = toasty::Db::builder()
    .register::<User>()
    .connect("dynamodb://localhost:8000")
    .await?;

db.push_schema().await?;
```

For the final remaining cataloged gap, continue with
[gaps-cassandra-driver.md](gaps-cassandra-driver.md).
