# Toasty Examples

## Running An Example
You can run the examples using an [sqlite](src/db/sqlite) in-memory database on most platforms using:

```
git clone https://github.com/tokio-rs/toasty.git
cd toasty
cargo run --example hello-toasty
```
You can also run the examples with an alternative database such as [dynamodb](src/db/ddb) through disabling the default feature (sqlite) and passing a feature for the database:
```
cargo run --example hello-toasty --no-default-features --feature dynamodb
```

## Creating An Example
You can create a new example for testing purposes by using the [toasty-cli](src/cli), bt providing an example name such as `test`:
```
cargo run --bin toasty-cli init-example test
```
Then modify the `schema.toasty` file in the `toasty-examples/examples/test` directory, through adding e.g.: 

```rust
model User {
    #[key]
    #[auto]
    id: Id,

    name: String,

    #[unique]
    email: String,
}
```
Then you can generate the code and run your new example like this: 
```
cargo run --bin toasty-cli gen-example test
cargo run --example test
```
We provide the following examples:

## Hello Toasty

## Crate Hub

## User Has One Profile

## Composite Key