//! A minimal, cargo-agnostic dependency graph.
//!
//! Deliberately decoupled from `cargo_metadata` types so the policy in
//! [`crate::deps_policy`] can be unit-tested by constructing a [`Graph`] by
//! hand, with no `cargo metadata` invocation involved.

/// One resolved package: a workspace crate or an external dependency.
#[derive(Debug, Clone, Default)]
pub struct PackageNode {
    /// The crate name as it appears in `Cargo.toml` / on crates.io.
    pub name: String,
    /// Directory relative to the workspace root, using `/` separators
    /// (e.g. `"crates/tabula-core"`, `"games/chess"`, `"xtask"`).
    /// `None` for packages that are not workspace members.
    pub rel_path: Option<String>,
    /// Names of dependencies declared directly in this package's manifest
    /// (normal, dev, and build dependencies combined). Only meaningful for
    /// workspace members; left empty for external packages.
    pub direct_deps: Vec<String>,
    /// Indices into the owning [`Graph::nodes`] for every dependency in the
    /// **resolved** graph (the actual, feature-activated dependency set).
    pub resolved_deps: Vec<usize>,
}

/// The whole resolved workspace dependency graph.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub nodes: Vec<PackageNode>,
}

/// What a node means to a [`Graph::find_forbidden`] walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// This node breaks the rule; report it and do not expand past it.
    Forbidden(String),
    /// This node is a sanctioned boundary (e.g. an unconstrained tier):
    /// stop expanding past it, but it is not itself a violation.
    Boundary,
    /// Keep walking into this node's dependencies.
    Continue,
}

impl Graph {
    /// Breadth-first search from `root`, skipping over (not expanding past)
    /// any node `classify` marks [`Verdict::Forbidden`] or [`Verdict::Boundary`].
    /// Returns one entry per distinct forbidden crate *name* reached, each
    /// with the shortest path from `root` (inclusive of both ends) and the
    /// reason `classify` gave for it.
    ///
    /// Stopping at the first forbidden node on a branch keeps the report
    /// focused: once `tabula-core -> renderer-macroquad` is reported, we do
    /// not also walk into `renderer-macroquad`'s own dependencies. Stopping
    /// at a boundary keeps a deliberately unconstrained crate's own
    /// dependency choices (e.g. a test harness pulling `tempfile`) from
    /// being blamed on whatever depends on it.
    pub fn find_forbidden(
        &self,
        root: usize,
        mut classify: impl FnMut(&PackageNode) -> Verdict,
    ) -> Vec<ForbiddenHit> {
        use std::collections::VecDeque;

        let mut visited = vec![false; self.nodes.len()];
        let mut seen_names = std::collections::BTreeSet::new();
        let mut queue = VecDeque::new();
        let mut hits = Vec::new();

        visited[root] = true;
        queue.push_back(vec![root]);

        while let Some(path) = queue.pop_front() {
            let cur = *path.last().expect("path is never empty");
            if cur != root {
                match classify(&self.nodes[cur]) {
                    Verdict::Forbidden(reason) => {
                        let name = self.nodes[cur].name.clone();
                        if seen_names.insert(name.clone()) {
                            hits.push(ForbiddenHit {
                                name,
                                reason,
                                path: path.iter().map(|&i| self.nodes[i].name.clone()).collect(),
                            });
                        }
                        continue;
                    }
                    Verdict::Boundary => continue,
                    Verdict::Continue => {}
                }
            }
            for &next in &self.nodes[cur].resolved_deps {
                if !visited[next] {
                    visited[next] = true;
                    let mut extended = path.clone();
                    extended.push(next);
                    queue.push_back(extended);
                }
            }
        }

        hits
    }

    /// Index of the workspace member with this crate name, if any.
    pub fn workspace_member_index(&self, name: &str) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.rel_path.is_some() && n.name == name)
    }
}

/// A forbidden node reached by [`Graph::find_forbidden`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenHit {
    pub name: String,
    pub reason: String,
    /// Crate names from the root (inclusive) to the offender (inclusive).
    pub path: Vec<String>,
}
