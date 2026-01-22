//! Test that Model associated types are accessible and correctly typed

use toasty::Model;

#[derive(Model)]
struct User {
    #[key]
    id: u64,
}

// Helper function to verify type identity at compile time
#[allow(dead_code)]
fn check_type<T>(_value: &T) {}

// Test that Model::Query can be used in generic contexts
#[allow(dead_code)]
fn use_model_query_in_generic<M: Model>(_query: M::Query) {}

#[test]
fn model_query_type_is_accessible() {
    // Get a query using the generated API
    let query = User::all();

    // Verify that the type matches Model::Query associated type
    check_type::<<User as Model>::Query>(&query);
}
