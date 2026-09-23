//! The reference graph between derived values, derived from references in the IR.
//!
//! The graph is a view, not a declaration (PRINCIPLES.md §8): edges come from `derived`
//! expression nodes. Cycles are found here, on names, because a cycle has no content hash.

use std::collections::{BTreeMap, BTreeSet};

use crate::admit::AdmissionError;
use crate::wire::{WDerived, WExpr, WExprKind};

/// Names of derived values referenced anywhere in `e`.
pub(crate) fn references(e: &WExpr, out: &mut BTreeSet<String>) {
    match &e.kind {
        WExprKind::Derived { name, .. } => {
            out.insert(name.clone());
        }
        WExprKind::Op { args, .. } => args.iter().for_each(|a| references(a, out)),
        WExprKind::In { arg, .. } | WExprKind::Wrap { arg, .. } => references(arg, out),
        WExprKind::Lit { .. } | WExprKind::Field { .. } | WExprKind::Param(_) => {}
    }
}

/// Topological order (dependencies first, ties by name), or one `CYCLE` error per cycle.
pub(crate) fn evaluation_order(derived: &[WDerived]) -> Result<Vec<String>, Vec<AdmissionError>> {
    let known: BTreeSet<String> = derived.iter().map(|d| d.name.clone()).collect();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut locs = BTreeMap::new();
    for d in derived {
        let mut refs = BTreeSet::new();
        references(&d.body, &mut refs);
        refs.retain(|r| known.contains(r));
        edges.entry(d.name.clone()).or_default().extend(refs);
        locs.entry(d.name.clone()).or_insert_with(|| d.loc.clone());
    }

    let cycles = find_cycles(&edges);
    if !cycles.is_empty() {
        return Err(cycles
            .into_iter()
            .map(|cycle| {
                let mut path = cycle.clone();
                if let Some(first) = cycle.first() {
                    path.push(first.clone());
                }
                let mut e = AdmissionError::new(
                    "CYCLE",
                    format!("derived values form a cycle: {}", path.join(" → ")),
                    cycle.first().and_then(|n| locs.get(n)),
                );
                e.related_locs = cycle.iter().filter_map(|n| locs.get(n).cloned()).collect();
                e
            })
            .collect());
    }

    // Kahn's algorithm; the ready set is ordered by name.
    let mut remaining: BTreeMap<String, usize> = edges
        .iter()
        .map(|(n, deps)| (n.clone(), deps.len()))
        .collect();
    let mut ready: BTreeSet<String> = remaining
        .iter()
        .filter(|(_, c)| **c == 0)
        .map(|(n, _)| n.clone())
        .collect();
    let mut order = Vec::new();
    while let Some(next) = ready.pop_first() {
        remaining.remove(&next);
        for (name, deps) in &edges {
            if deps.contains(&next)
                && let Some(c) = remaining.get_mut(name)
            {
                *c -= 1;
                if *c == 0 {
                    ready.insert(name.clone());
                }
            }
        }
        order.push(next);
    }
    Ok(order)
}

/// Each cycle once, as a path starting at its smallest name.
fn find_cycles(edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let mut cycles = Vec::new();
    let mut covered: BTreeSet<String> = BTreeSet::new();
    for start in edges.keys() {
        if covered.contains(start) {
            continue;
        }
        if let Some(path) = path_back_to(start, edges, &covered) {
            covered.extend(path.iter().cloned());
            cycles.push(path);
        }
    }
    cycles
}

/// Depth-first search from `start` for a path that returns to `start`, visiting names in
/// order and never entering nodes of already reported cycles.
fn path_back_to(
    start: &str,
    edges: &BTreeMap<String, BTreeSet<String>>,
    covered: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut stack: Vec<(String, Vec<String>)> = vec![(start.to_string(), vec![start.to_string()])];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some((node, path)) = stack.pop() {
        let Some(nexts) = edges.get(&node) else {
            continue;
        };
        // Push in reverse so the smallest name is explored first.
        for next in nexts.iter().rev() {
            if next == start {
                return Some(path.clone());
            }
            if covered.contains(next) || path.contains(next) || !seen.insert(next.clone()) {
                continue;
            }
            let mut p = path.clone();
            p.push(next.clone());
            stack.push((next.clone(), p));
        }
    }
    None
}
