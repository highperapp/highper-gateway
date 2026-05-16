//! GraphQL query depth + complexity analyzer (B5 — Workstream 0.E).
//!
//! Walks an `async_graphql_parser::types::ExecutableDocument` to compute:
//!
//! - **Depth** — the maximum nesting level reached among any operation's
//!   selection sets. A flat query `{ a b c }` is depth 1; `{ a { b } }`
//!   is depth 2.
//! - **Complexity** — the total field-selection count visited across all
//!   operations (with fragment expansion). Each field selected anywhere
//!   in the query contributes 1 to complexity. Per-field weighting via
//!   `@cost` directives is queued for a future enhancement.
//!
//! Fragment cycles are detected and rejected — the parser's own check is
//! advisory; this analyzer must not be tricked into unbounded recursion.

use async_graphql_parser::types::{
    DocumentOperations, ExecutableDocument, FragmentDefinition, Selection, SelectionSet,
};
use async_graphql_parser::Positioned;
use async_graphql_value::Name;
use std::collections::{HashMap, HashSet};

/// The pure stats produced by [`analyze`]. The caller decides how to
/// compare these against operator-configured thresholds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalysisStats {
    pub depth: u32,
    pub complexity: u32,
}

/// Reasons the analyzer can refuse a query without ever reaching the
/// threshold-comparison step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    /// A fragment spread referenced a fragment not present in the
    /// document.
    UnknownFragment { name: String },
    /// A fragment cycle was detected (e.g., `fragment A on T { ...B }`
    /// and `fragment B on T { ...A }`).
    FragmentCycle { name: String },
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::UnknownFragment { name } => {
                write!(f, "fragment `{}` referenced but not defined", name)
            }
            AnalysisError::FragmentCycle { name } => {
                write!(f, "fragment `{}` is part of a cycle", name)
            }
        }
    }
}

impl std::error::Error for AnalysisError {}

/// Compute depth + complexity for every operation in the document and
/// return the maxima. Fragments are inlined with cycle detection.
pub fn analyze(doc: &ExecutableDocument) -> Result<AnalysisStats, AnalysisError> {
    let mut overall = AnalysisStats::default();

    let mut walk_op = |op_selection_set: &SelectionSet| -> Result<(), AnalysisError> {
        let mut visited: HashSet<&Name> = HashSet::new();
        let mut op_stats = AnalysisStats::default();
        walk_selection_set(
            op_selection_set,
            &doc.fragments,
            &mut visited,
            0,
            &mut op_stats,
        )?;
        overall.depth = overall.depth.max(op_stats.depth);
        overall.complexity = overall.complexity.saturating_add(op_stats.complexity);
        Ok(())
    };

    match &doc.operations {
        DocumentOperations::Single(op) => walk_op(&op.node.selection_set.node)?,
        DocumentOperations::Multiple(ops) => {
            for op in ops.values() {
                walk_op(&op.node.selection_set.node)?;
            }
        }
    }

    Ok(overall)
}

fn walk_selection_set<'a>(
    set: &'a SelectionSet,
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    visited_fragments: &mut HashSet<&'a Name>,
    current_depth: u32,
    stats: &mut AnalysisStats,
) -> Result<(), AnalysisError> {
    if set.items.is_empty() {
        return Ok(());
    }

    // Entering a non-empty selection set increases depth by 1.
    let new_depth = current_depth.saturating_add(1);
    stats.depth = stats.depth.max(new_depth);

    for selection in &set.items {
        match &selection.node {
            Selection::Field(field) => {
                stats.complexity = stats.complexity.saturating_add(1);
                walk_selection_set(
                    &field.node.selection_set.node,
                    fragments,
                    visited_fragments,
                    new_depth,
                    stats,
                )?;
            }
            Selection::InlineFragment(frag) => {
                // Inline fragments don't count toward depth on their own
                // (they're a type-narrowing wrapper, not a nesting layer);
                // we walk their selection set at the *same* depth.
                walk_inline_set(
                    &frag.node.selection_set.node,
                    fragments,
                    visited_fragments,
                    new_depth,
                    stats,
                )?;
            }
            Selection::FragmentSpread(spread) => {
                let name = &spread.node.fragment_name.node;
                if visited_fragments.contains(name) {
                    return Err(AnalysisError::FragmentCycle {
                        name: name.to_string(),
                    });
                }
                let def = fragments
                    .get(name)
                    .ok_or_else(|| AnalysisError::UnknownFragment {
                        name: name.to_string(),
                    })?;
                visited_fragments.insert(name);
                walk_inline_set(
                    &def.node.selection_set.node,
                    fragments,
                    visited_fragments,
                    new_depth,
                    stats,
                )?;
                visited_fragments.remove(name);
            }
        }
    }

    Ok(())
}

/// Walk a fragment's selection set without re-incrementing depth (the
/// caller is already at the right level).
fn walk_inline_set<'a>(
    set: &'a SelectionSet,
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    visited_fragments: &mut HashSet<&'a Name>,
    current_depth: u32,
    stats: &mut AnalysisStats,
) -> Result<(), AnalysisError> {
    for selection in &set.items {
        match &selection.node {
            Selection::Field(field) => {
                stats.complexity = stats.complexity.saturating_add(1);
                walk_selection_set(
                    &field.node.selection_set.node,
                    fragments,
                    visited_fragments,
                    current_depth,
                    stats,
                )?;
            }
            Selection::InlineFragment(frag) => {
                walk_inline_set(
                    &frag.node.selection_set.node,
                    fragments,
                    visited_fragments,
                    current_depth,
                    stats,
                )?;
            }
            Selection::FragmentSpread(spread) => {
                let name = &spread.node.fragment_name.node;
                if visited_fragments.contains(name) {
                    return Err(AnalysisError::FragmentCycle {
                        name: name.to_string(),
                    });
                }
                let def = fragments
                    .get(name)
                    .ok_or_else(|| AnalysisError::UnknownFragment {
                        name: name.to_string(),
                    })?;
                visited_fragments.insert(name);
                walk_inline_set(
                    &def.node.selection_set.node,
                    fragments,
                    visited_fragments,
                    current_depth,
                    stats,
                )?;
                visited_fragments.remove(name);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql_parser::parse_query;

    fn parse(s: &str) -> ExecutableDocument {
        parse_query(s).expect("parse failed")
    }

    #[test]
    fn flat_query_is_depth_1() {
        let doc = parse("{ a b c }");
        let stats = analyze(&doc).unwrap();
        assert_eq!(stats.depth, 1);
        assert_eq!(stats.complexity, 3);
    }

    #[test]
    fn nested_query_depth_increments() {
        let doc = parse("{ a { b { c } } }");
        let stats = analyze(&doc).unwrap();
        assert_eq!(stats.depth, 3);
        assert_eq!(stats.complexity, 3);
    }

    #[test]
    fn complexity_sums_siblings_and_descendants() {
        let doc = parse("{ a b { x y z } c }");
        let stats = analyze(&doc).unwrap();
        // a, b, x, y, z, c = 6 fields
        assert_eq!(stats.complexity, 6);
        // depth: { … } = 1, b's children = 2
        assert_eq!(stats.depth, 2);
    }

    #[test]
    fn named_fragment_is_inlined() {
        let q = r#"
            query Q { user { ...Fields } }
            fragment Fields on User { name email }
        "#;
        let doc = parse(q);
        let stats = analyze(&doc).unwrap();
        // user, name, email = 3
        assert_eq!(stats.complexity, 3);
        // { user { name email } } => depth 2
        assert_eq!(stats.depth, 2);
    }

    #[test]
    fn fragment_cycle_is_rejected() {
        let q = r#"
            { ...A }
            fragment A on Q { ...B }
            fragment B on Q { ...A }
        "#;
        let doc = parse(q);
        let err = analyze(&doc).unwrap_err();
        assert!(matches!(err, AnalysisError::FragmentCycle { .. }));
    }

    #[test]
    fn unknown_fragment_is_rejected() {
        let q = "{ ...Missing }";
        let doc = parse(q);
        let err = analyze(&doc).unwrap_err();
        assert!(matches!(err, AnalysisError::UnknownFragment { .. }));
    }

    #[test]
    fn inline_fragment_does_not_add_extra_depth_layer() {
        // `{ a { ... on T { b } } }` should be depth 2 (the inline
        // fragment is a type-narrowing wrapper, not a nesting level).
        let doc = parse("{ a { ... on T { b } } }");
        let stats = analyze(&doc).unwrap();
        assert_eq!(stats.depth, 2);
        assert_eq!(stats.complexity, 2); // a, b
    }

    #[test]
    fn multiple_named_operations_take_max_depth_sum_complexity() {
        let q = r#"
            query First { a { b { c } } }
            query Second { x }
        "#;
        let doc = parse(q);
        let stats = analyze(&doc).unwrap();
        assert_eq!(stats.depth, 3); // max of First and Second
        assert_eq!(stats.complexity, 4); // a+b+c+x
    }
}
