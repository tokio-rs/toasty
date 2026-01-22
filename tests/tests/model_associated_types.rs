//! Test that Model associated types are accessible and correctly typed

use toasty::Model;

#[derive(Model)]
struct User {
    #[key]
    id: u64,
}

// Type aliases for User's associated types
#[allow(dead_code)]
type UserQuery = <User as Model>::Query;
#[allow(dead_code)]
type UserCreate = <User as Model>::Create;
#[allow(dead_code)]
type UserUpdate<'a> = <User as Model>::Update<'a>;
#[allow(dead_code)]
type UserUpdateQuery = <User as Model>::UpdateQuery;

// Helper function to verify type identity at compile time
#[allow(dead_code)]
fn check_type<T>(_value: &T) {}

// Test that Model::Query can be used in generic contexts
#[allow(dead_code)]
fn use_model_query_in_generic<M: Model>(_query: M::Query) {}

// Test that Model::Create can be used in generic contexts
#[allow(dead_code)]
fn use_model_create_in_generic<M: Model>(_create: M::Create) {}

// Test that Model::Update can be used in generic contexts
#[allow(dead_code)]
fn use_model_update_in_generic<M: Model>(_update: M::Update<'_>) {}

// Test that Model::UpdateQuery can be used in generic contexts
#[allow(dead_code)]
fn use_model_update_query_in_generic<M: Model>(_update_query: M::UpdateQuery) {}

#[test]
fn model_query_type_is_accessible() {
    // Get a query using the generated API
    let query = User::all();

    // Verify that the type matches Model::Query associated type
    check_type::<<User as Model>::Query>(&query);
}

#[test]
fn model_create_type_is_accessible() {
    // Get a create builder using the generated API
    let create = User::create();

    // Verify that the type matches Model::Create associated type
    check_type::<<User as Model>::Create>(&create);
}

#[test]
fn model_update_type_is_accessible() {
    // Get an update builder using the generated API
    let mut user = User { id: 1 };
    let update = user.update();

    // Verify that the type matches Model::Update associated type
    check_type::<<User as Model>::Update<'_>>(&update);
}

#[test]
fn model_update_query_type_is_accessible() {
    // Get an update query using the generated API
    let update_query = User::all().update();

    // Verify that the type matches Model::UpdateQuery associated type
    check_type::<<User as Model>::UpdateQuery>(&update_query);
}
