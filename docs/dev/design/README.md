# Design

Guide-level design documents for Toasty features.

A design document describes a change from the perspective of its
audience: Toasty users (Rust developers writing models and queries) and
driver implementors. It covers the public API, runtime behavior, edge
cases, and driver-integration contract. It does not cover internal
implementation choices.

## Adding a new design document

The recommended path for non-trivial features (see
[`CONTRIBUTING.md`](../../../CONTRIBUTING.md)):

1. Open a feature-proposal issue.
2. Land a PR that adds an entry to [`../roadmap/`](../roadmap/) **and** a
   design doc here. Start from [`_template.md`](./_template.md). Both go
   in the same PR.
3. Once the design lands, submit the implementation in a follow-up PR.

## Existing design documents

See [`../README.md`](../README.md#design-documents) for the index.
