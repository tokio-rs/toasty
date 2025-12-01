use bson::Document;

pub fn build_match_stage(_filter: &Document) -> Document {
    todo!("build aggregation match stage")
}

pub fn build_project_stage(_fields: &[String]) -> Document {
    todo!("build aggregation project stage")
}

pub fn build_sort_stage(_sort_fields: &[(String, i32)]) -> Document {
    todo!("build aggregation sort stage")
}
