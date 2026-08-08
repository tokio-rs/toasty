# DynamoDB Driver Notes

DynamoDB has features that post-date the model's training data. Fetch current AWS
documentation before implementing or designing DynamoDB driver features rather than
relying on prior knowledge.

## Multi-attribute GSI keys

GSIs now support composite keys built from multiple attributes. A partition key can be
composed of up to 4 attributes and a sort key up to 4 attributes, for up to 8 attributes
per key schema total. Previously a GSI key was a single attribute (partition key) or two
attributes (partition key + sort key).

DynamoDB hashes the partition key attributes together for distribution and maintains
hierarchical sort order across the sort key attributes. This eliminates the need to
concatenate attributes into synthetic keys like `"TOURNAMENT#WINTER2024#REGION#NA-EAST"`.

**Querying:** All partition key attributes must be specified using equality conditions.
Sort key attributes can be queried left-to-right in declaration order — you can query
the first attribute alone, the first two together, etc., but cannot skip attributes.
Inequality conditions (`>`, `<`, `BETWEEN`, `begins_with`) must appear last.

**Key attribute types:** Each attribute in a multi-attribute key can be `String`,
`Number`, or `Binary`. `Number` sorts numerically; `String` sorts lexicographically
(zero-pad if natural numeric order matters).

Reference: https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GSI.html#GSI.MultiAttributeKeys
