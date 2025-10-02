use super::{eval, plan, Exec, Result};
use std::collections::HashMap;
use toasty_core::stmt;
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &plan::NestedMerge) -> Result<()> {
        // Load root materialization
        let root_records = self.vars.load(action.root).collect().await?;

        // Load all data needed for nested levels
        let mut all_data = HashMap::new();
        self.load_nested_data(&action.nested, &mut all_data).await?;

        // Build all indexes upfront using the pre-planned index specifications
        let mut all_indexes = HashMap::new();
        for (var_id, index_columns) in &action.indexes {
            let data = all_data
                .get(var_id)
                .expect("Data should be loaded for all indexed VarIds");
            let index = self.build_hash_index(data, index_columns)?;
            all_indexes.insert(*var_id, index);
        }

        // Execute the hierarchical nested merge with pre-built indexes
        let results = self
            .execute_nested_levels(
                root_records,
                &action.nested,
                &action.projection,
                &[], // Empty ancestor stack for root level
                &all_data,
                &all_indexes,
            )
            .await?;

        // Store output
        self.vars.store(action.output, ValueStream::from_vec(results));
        Ok(())
    }

    /// Load all data from nested tree
    fn load_nested_data<'a>(
        &'a mut self,
        nested_levels: &'a [plan::NestedLevel],
        all_data: &'a mut HashMap<plan::VarId, Vec<stmt::Value>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + 'a>> {
        Box::pin(async move {
            for level in nested_levels {
                if !all_data.contains_key(&level.source) {
                    let data = self.vars.load(level.source).collect().await?;
                    all_data.insert(level.source, data);
                }

                if !level.nested.is_empty() {
                    self.load_nested_data(&level.nested, all_data).await?;
                }
            }
            Ok(())
        })
    }

    /// Recursively execute nested merges at all levels
    ///
    /// This processes one level of nesting, then recursively processes children.
    /// Execution is outside-in to provide ancestor context.
    /// All data and indexes are pre-loaded to avoid rebuilding during iteration.
    fn execute_nested_levels<'a>(
        &'a self,
        parent_records: Vec<stmt::Value>,
        nested_levels: &'a [plan::NestedLevel],
        projection: &'a eval::Func,
        ancestor_stack: &'a [stmt::Value],
        all_data: &'a HashMap<plan::VarId, Vec<stmt::Value>>,
        all_indexes: &'a HashMap<plan::VarId, HashMap<CompositeKey, Vec<stmt::Value>>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<stmt::Value>>> + 'a>> {
        Box::pin(async move {
        // Prepare loaded data for this level
        let mut loaded_levels = Vec::with_capacity(nested_levels.len());

        for level in nested_levels {
            let nested_data = all_data
                .get(&level.source)
                .expect("Data should be loaded for all VarIds");

            loaded_levels.push(LoadedNestedLevel {
                data: nested_data,
                level_info: level,
            });
        }

        // Process each parent record
        let mut results = Vec::with_capacity(parent_records.len());

        for parent_record in parent_records {
            // Build ancestor context stack: [grandparents..., parent]
            let mut context_stack = ancestor_stack.to_vec();
            context_stack.push(parent_record.clone());

            // Filter and process all nested collections for this parent
            let mut filtered_collections = Vec::new();

            for loaded_level in &loaded_levels {
                // Filter using ancestor context
                let filtered = self.filter_hierarchical(
                    loaded_level.data,
                    &context_stack,
                    &loaded_level.level_info.qualification,
                    all_indexes,
                )?;

                // If this level has children, recursively merge them
                let processed = if !loaded_level.level_info.nested.is_empty() {
                    self.execute_nested_levels(
                        filtered,
                        &loaded_level.level_info.nested,
                        &loaded_level.level_info.projection,
                        &context_stack, // Pass down ancestor context
                        all_data,       // Pass through pre-loaded data
                        all_indexes,    // Pass through pre-built indexes
                    )
                    .await?
                } else {
                    // Leaf level - just apply projection to each record
                    filtered
                        .iter()
                        .map(|rec| loaded_level.level_info.projection.eval(&[rec.clone()]))
                        .collect::<Result<Vec<_>>>()?
                };

                filtered_collections.push(stmt::Value::List(processed));
            }

            // Apply projection at this level: [parent_record, filtered_0, filtered_1, ...]
            // Collections may not be in arg_index order, so build a sparse array
            let max_arg = loaded_levels
                .iter()
                .map(|l| l.level_info.arg_index)
                .max()
                .unwrap_or(0);
            let mut projection_args = vec![stmt::Value::Null; max_arg + 2]; // +1 for parent, +1 for 0-indexing
            projection_args[0] = parent_record;

            for (loaded_level, filtered) in loaded_levels.iter().zip(filtered_collections) {
                projection_args[loaded_level.level_info.arg_index + 1] = filtered;
            }

            let projected = projection.eval(&projection_args[..])?;
            results.push(projected);
        }

        Ok(results)
        })
    }

    /// Filter nested records using ancestor context
    ///
    /// The qualification can reference ANY ancestor in the context stack,
    /// not just the immediate parent.
    fn filter_hierarchical(
        &self,
        nested_records: &[stmt::Value],
        ancestor_stack: &[stmt::Value], // [root, child, grandchild, ..., parent]
        qualification: &plan::MergeQualification,
        all_indexes: &HashMap<plan::VarId, HashMap<CompositeKey, Vec<stmt::Value>>>,
    ) -> Result<Vec<stmt::Value>> {
        match qualification {
            plan::MergeQualification::Equality {
                root_columns,
                index_id,
            } => {
                // Build composite key from ancestor stack
                // root_columns = [(levels_up, col_idx), ...]
                let mut key_values = Vec::new();

                for (levels_up, col_idx) in root_columns {
                    // levels_up: 0 = immediate parent, 1 = grandparent, etc.
                    let ancestor_idx = ancestor_stack.len() - 1 - levels_up;
                    let ancestor_record = ancestor_stack[ancestor_idx].expect_record();
                    key_values.push(ancestor_record[*col_idx].clone());
                }

                let key = CompositeKey(key_values);

                // Lookup in pre-built index using index_id
                let index = all_indexes
                    .get(index_id)
                    .expect("Hash index should exist for Equality qualification");

                Ok(index.get(&key).cloned().unwrap_or_default())
            }
            plan::MergeQualification::Predicate(predicate) => {
                // Evaluate predicate for each nested record
                // Args: [ancestor_stack..., nested_record] -> bool
                let mut matches = Vec::new();

                for nested_record in nested_records {
                    let mut args = ancestor_stack.to_vec();
                    args.push(nested_record.clone());

                    if predicate.eval_bool(&args[..])? {
                        matches.push(nested_record.clone());
                    }
                }

                Ok(matches)
            }
        }
    }

    fn build_hash_index(
        &self,
        records: &[stmt::Value],
        key_columns: &[usize],
    ) -> Result<HashMap<CompositeKey, Vec<stmt::Value>>> {
        let mut index = HashMap::new();

        for record in records {
            let record_inner = record.expect_record();
            let key = self.extract_key(record_inner, key_columns)?;
            index
                .entry(key)
                .or_insert_with(Vec::new)
                .push(record.clone());
        }

        Ok(index)
    }

    fn extract_key(
        &self,
        record: &stmt::ValueRecord,
        columns: &[usize],
    ) -> Result<CompositeKey> {
        let values: Vec<_> = columns
            .iter()
            .map(|&col_idx| record[col_idx].clone())
            .collect();
        Ok(CompositeKey(values))
    }
}

// Helper struct for loaded nested levels
struct LoadedNestedLevel<'a> {
    data: &'a Vec<stmt::Value>,
    level_info: &'a plan::NestedLevel,
}

// Composite key type for multi-column equality
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompositeKey(Vec<stmt::Value>);
