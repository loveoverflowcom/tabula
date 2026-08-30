//! `cargo xtask check-deps` — the shell around [`crate::deps_policy`].
//!
//! Resolves the real workspace graph via `cargo metadata` (with every
//! feature enabled, so a dependency that only appears behind a feature flag
//! is still caught), converts it into the generic [`crate::graph::Graph`],
//! and runs the pure policy over it.

use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};

use crate::deps_policy::{Matrix, MatrixError};
use crate::graph::{Graph, PackageNode};

#[derive(Debug, thiserror::Error)]
pub enum CheckDepsError {
    #[error("reading deps.toml: {0}")]
    ReadMatrix(#[source] std::io::Error),
    #[error(transparent)]
    ParseMatrix(#[from] MatrixError),
    #[error("running `cargo metadata`: {0}")]
    Metadata(#[from] cargo_metadata::Error),
}

pub fn run() -> Result<bool, CheckDepsError> {
    let metadata = MetadataCommand::new()
        .features(CargoOpt::AllFeatures)
        .exec()?;

    let matrix_path = metadata.workspace_root.join("deps.toml");
    let matrix_src =
        std::fs::read_to_string(matrix_path.as_std_path()).map_err(CheckDepsError::ReadMatrix)?;
    let matrix = Matrix::parse(&matrix_src)?;

    let graph = build_graph(&metadata);
    let violations = matrix.evaluate(&graph);

    let checked = graph.nodes.iter().filter(|n| n.rel_path.is_some()).count();
    if violations.is_empty() {
        println!("check-deps: {checked} workspace crates checked against deps.toml — all clear");
        Ok(true)
    } else {
        for v in &violations {
            eprintln!("{v}\n");
        }
        eprintln!(
            "check-deps: {} violation(s) across {checked} workspace crates",
            violations.len()
        );
        Ok(false)
    }
}

fn build_graph(metadata: &Metadata) -> Graph {
    let workspace_root = &metadata.workspace_root;
    let members: std::collections::HashSet<&cargo_metadata::PackageId> =
        metadata.workspace_members.iter().collect();

    let mut id_to_idx = std::collections::HashMap::with_capacity(metadata.packages.len());
    let mut nodes = Vec::with_capacity(metadata.packages.len());

    for (i, pkg) in metadata.packages.iter().enumerate() {
        id_to_idx.insert(pkg.id.clone(), i);

        let rel_path = if members.contains(&pkg.id) {
            pkg.manifest_path
                .parent()
                .and_then(|dir| dir.strip_prefix(workspace_root).ok())
                .map(|p| p.as_str().replace('\\', "/"))
        } else {
            None
        };

        let mut direct: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for dep in &pkg.dependencies {
            direct.insert(dep.name.clone());
        }

        nodes.push(PackageNode {
            name: pkg.name.clone(),
            rel_path,
            direct_deps: direct.into_iter().collect(),
            resolved_deps: Vec::new(),
        });
    }

    if let Some(resolve) = &metadata.resolve {
        for resolved in &resolve.nodes {
            let Some(&idx) = id_to_idx.get(&resolved.id) else {
                continue;
            };
            // Architecture bans (I-1, I-4) are about what SHIPS. An edge that
            // exists only to build this package's own test binaries (a
            // `[dev-dependencies]`-only path) must not trip a transitive-ban
            // check — e.g. every game's unconditional dev-dependency on
            // `tabula-testkit` must not fail because `testkit`'s own
            // `presentation` feature (turned on by `--all-features`) pulls in
            // `insta` -> `tempfile` -> `getrandom`. Keep an edge if ANY of
            // its declared kinds is normal or build.
            let deps = resolved
                .deps
                .iter()
                .filter(|d| is_production_edge(&d.dep_kinds))
                .filter_map(|d| id_to_idx.get(&d.pkg).copied())
                .collect();
            nodes[idx].resolved_deps = deps;
        }
    }

    Graph { nodes }
}

fn is_production_edge(kinds: &[cargo_metadata::DepKindInfo]) -> bool {
    use cargo_metadata::DependencyKind;
    kinds.is_empty() || kinds.iter().any(|k| k.kind != DependencyKind::Development)
}
