# Git Commit Guidelines

Toasty follows the [Conventional Commits][cc] specification. Precise rules
about commit message formatting lead to **more readable messages** that are
easy to follow when looking through the **project history**, and let us use
commit messages to **generate the change log**.

[cc]: https://www.conventionalcommits.org/

## Commit Message Format

Each commit message consists of a **header**, an optional **body**, and an
optional **footer**. The header has a special format that includes a
**type**, an optional **scope**, and a **subject**:

```
<type>(<scope>): <subject>
<BLANK LINE>
<body>
<BLANK LINE>
<footer>
```

No line of the commit message should be longer than 100 characters. This
keeps messages readable on GitHub and in various git tools.

The PR title must follow the same format — it becomes the squash-merge
commit message.

## Type

Must be one of the following:

* **feat**: a new feature
* **fix**: a bug fix
* **docs**: documentation only changes
* **style**: changes that do not affect the meaning of the code
  (white-space, formatting, missing semi-colons, etc.)
* **refactor**: a code change that neither fixes a bug nor adds a feature
* **perf**: a code change that improves performance
* **test**: adding or correcting tests
* **build**: changes to the build system or external dependencies
* **ci**: changes to CI configuration and scripts
* **chore**: changes to auxiliary tooling that do not affect source or
  tests
* **revert**: reverts a previous commit

## Scope

The scope is optional. When present, it should refer to the area of
Toasty being touched. Common scopes:

* `core` — `toasty-core` (schema, statement AST, driver interface)
* `macros` — `toasty-macros` (`#[derive(Model)]`, `#[derive(Embed)]`)
* `sql` — `toasty-sql` (SQL serialization)
* `engine` — the query engine inside `toasty` (simplify, lower, plan, exec)
* `sqlite`, `postgresql`, `mysql`, `dynamodb` — individual drivers
* `tests` — integration test suite (`toasty-driver-integration-suite`)
* `docs` — only when `type` is not already `docs`

Omit the scope when the change does not fit cleanly into one area, or
when the type alone is descriptive enough (e.g., `chore: bump
dependencies`).

## Subject

The subject contains a succinct description of the change:

* use the imperative, present tense: "add" not "added" nor "adds"
* begin with a lowercase letter
* no dot (`.`) at the end

## Body

Just as in the **subject**, use the imperative, present tense. The body
should include the motivation for the change and contrast it with
previous behavior. Most commits do not need a body — add one when the
"what" is not self-evident from the subject, or when the "why" would not
be obvious to a future reader.

## Footer

The footer is the place to reference GitHub issues that this commit
**Closes** (`Closes #123`) and to note any **Breaking Changes**.

The last line of a commit that introduces a breaking change should be in
the form:

```
BREAKING CHANGE: <description of what breaks and how to migrate>
```

## Examples

```
feat(engine): simplify uniform-arms Match into a projection

When every arm of a Match produces the same shape, projection can be
pushed through the Match and the arms folded together. This enables
the planner to emit a single SQL expression for enum-over-primitive
comparisons instead of a CASE.
```

```
fix(sqlite): quote reserved identifiers in column definitions
```

```
docs: link COMMITS.md from CONTRIBUTING
```

```
refactor(core)!: rename `Schema::apps` to `Schema::models`

BREAKING CHANGE: `Schema::apps()` is now `Schema::models()`. Call sites
must be updated; the old name is not re-exported.
```
