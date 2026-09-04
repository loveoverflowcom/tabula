//! Pure policy evaluation for `xtask check-deps` (I-1, I-15).
//!
//! This module knows nothing about `cargo metadata`; it operates on the
//! generic [`Graph`] from [`crate::graph`]. That split is what makes the
//! policy itself unit-testable without shelling out to cargo (see the tests
//! at the bottom of this file).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;

use crate::graph::{Graph, PackageNode, Verdict};

/// `deps.toml`, deserialized.
#[derive(Debug, Deserialize)]
pub struct Matrix {
    #[serde(default)]
    pub banned: Banned,
    #[serde(default)]
    pub tiers: Tiers,
    #[serde(default)]
    pub categories: Categories,
    #[serde(rename = "crate", default)]
    pub crates: BTreeMap<String, CrateRule>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Banned {
    #[serde(default)]
    pub crates: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Tiers {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub unconstrained: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Categories {
    #[serde(default)]
    pub by_name: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub by_path_prefix: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct CrateRule {
    pub tier: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub feature: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub forbid: Vec<String>,
    #[serde(default)]
    pub forbidden_categories: Vec<String>,
    #[serde(default)]
    pub allow_games: bool,
}

/// One architecture violation, formatted the way `AGENTS.md` asks for:
/// actionable, and naming the *path* for transitive hits rather than making
/// the reader run `cargo tree`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub subject: String,
    pub offender: String,
    pub rule: String,
    pub path: Vec<String>,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "architecture violation:")?;
        writeln!(f, "  crate: {}", self.subject)?;
        writeln!(f, "  forbidden dependency: {}", self.offender)?;
        write!(f, "  rule: {}", self.rule)?;
        if self.path.len() > 2 {
            write!(f, "\n  path: {}", self.path.join(" -> "))?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MatrixError {
    #[error("failed to parse deps.toml: {0}")]
    Parse(#[from] toml::de::Error),
}

impl Matrix {
    pub fn parse(src: &str) -> Result<Self, MatrixError> {
        Ok(toml::from_str(src)?)
    }

    /// Find the rule that governs a workspace crate, matching (in order) by
    /// exact package name, exact directory path, then the `"games/*"`
    /// glob for anything under `games/`.
    pub fn rule_for<'a>(&'a self, name: &str, rel_path: &str) -> Option<(&'a str, &'a CrateRule)> {
        if let Some((k, r)) = self.crates.get_key_value(name) {
            return Some((k, r));
        }
        if let Some((k, r)) = self.crates.get_key_value(rel_path) {
            return Some((k, r));
        }
        if rel_path.starts_with("games/") {
            if let Some((k, r)) = self.crates.get_key_value("games/*") {
                return Some((k, r));
            }
        }
        None
    }

    fn is_category_member(&self, category: &str, node: &PackageNode) -> bool {
        if let Some(names) = self.categories.by_name.get(category) {
            if names.iter().any(|n| n == &node.name) {
                return true;
            }
        }
        if let Some(rel) = node.rel_path.as_deref() {
            if let Some(prefixes) = self.categories.by_path_prefix.get(category) {
                return prefixes
                    .iter()
                    .any(|p| rel == p.as_str() || rel.starts_with(&format!("{p}/")));
            }
        }
        false
    }

    fn tier_rank(&self, tier: &str) -> Option<usize> {
        self.tiers.order.iter().position(|t| t == tier)
    }

    fn is_unconstrained(&self, tier: &str) -> bool {
        self.tiers.unconstrained.iter().any(|t| t == tier)
    }

    fn layering_violation(
        &self,
        from_name: &str,
        from: &CrateRule,
        to_name: &str,
        to: &CrateRule,
    ) -> Option<String> {
        if self.is_unconstrained(&from.tier) || self.is_unconstrained(&to.tier) {
            return None;
        }
        let (from_rank, to_rank) = (self.tier_rank(&from.tier)?, self.tier_rank(&to.tier)?);
        if to_rank > from_rank {
            Some(format!(
                "dependency arrows must point down: {from_name} (tier {}) may not depend on {to_name} (tier {})",
                from.tier, to.tier
            ))
        } else {
            None
        }
    }

    /// Walk every workspace member in `graph` and report every violation of
    /// the matrix: disallowed direct dependencies, banned/forbidden-category
    /// crates reachable transitively, and tier layering breaks.
    pub fn evaluate(&self, graph: &Graph) -> Vec<Violation> {
        let mut out = Vec::new();

        for (idx, node) in graph.nodes.iter().enumerate() {
            let Some(rel_path) = node.rel_path.as_deref() else {
                continue; // not a workspace member
            };
            let Some((rule_key, rule)) = self.rule_for(&node.name, rel_path) else {
                out.push(Violation {
                    subject: node.name.clone(),
                    offender: "(none)".to_string(),
                    rule: format!(
                        "{} is a workspace crate with no entry in deps.toml — add one before merging",
                        node.name
                    ),
                    path: Vec::new(),
                });
                continue;
            };

            Self::check_direct_deps(node, rule_key, rule, graph, &mut out);
            self.check_transitive(idx, node, rule, graph, &mut out);
            self.check_layering(node, rule, graph, &mut out);
        }

        out
    }

    fn check_direct_deps(
        node: &PackageNode,
        rule_key: &str,
        rule: &CrateRule,
        graph: &Graph,
        out: &mut Vec<Violation>,
    ) {
        if rule.allow.iter().any(|a| a == "*") {
            return; // tooling tier: unconstrained by design (never shipped)
        }

        let mut allowed: BTreeSet<&str> = rule.allow.iter().map(String::as_str).collect();
        for extra in rule.feature.values() {
            allowed.extend(extra.iter().map(String::as_str));
        }

        for dep in &node.direct_deps {
            if allowed.contains(dep.as_str()) {
                continue;
            }
            if rule.allow_games && Self::is_game_crate(dep, graph) {
                continue;
            }
            out.push(Violation {
                subject: node.name.clone(),
                offender: dep.clone(),
                rule: format!(
                    "{} may declare dependencies only from its deps.toml allow-list (rule: crate.\"{rule_key}\")",
                    node.name
                ),
                path: Vec::new(),
            });
        }
    }

    fn is_game_crate(name: &str, graph: &Graph) -> bool {
        graph.nodes.iter().any(|n| {
            n.name == name
                && n.rel_path
                    .as_deref()
                    .is_some_and(|p| p.starts_with("games/"))
        })
    }

    fn check_transitive(
        &self,
        idx: usize,
        node: &PackageNode,
        rule: &CrateRule,
        graph: &Graph,
        out: &mut Vec<Violation>,
    ) {
        let mut forbidden_names: BTreeSet<&str> = rule.forbid.iter().map(String::as_str).collect();
        if rule.tier == "deterministic" {
            forbidden_names.extend(self.banned.crates.iter().map(String::as_str));
        }

        if forbidden_names.is_empty() && rule.forbidden_categories.is_empty() {
            return;
        }

        let hits = graph.find_forbidden(idx, |candidate| {
            if forbidden_names.contains(candidate.name.as_str()) {
                return Verdict::Forbidden(format!(
                    "{} must not depend on `{}` (explicit ban)",
                    node.name, candidate.name
                ));
            }
            for category in &rule.forbidden_categories {
                if self.is_category_member(category, candidate) {
                    return Verdict::Forbidden(format!(
                        "{} must not depend on {} ({category} is a forbidden category in deps.toml)",
                        node.name, candidate.name,
                    ));
                }
            }
            // Crossing into a deliberately unconstrained tier (test/tooling)
            // is itself the sanctioned boundary — do not chase its own
            // dependency choices back onto whatever depends on it.
            if let Some(rel) = candidate.rel_path.as_deref() {
                if let Some((_, candidate_rule)) = self.rule_for(&candidate.name, rel) {
                    if self.is_unconstrained(&candidate_rule.tier) {
                        return Verdict::Boundary;
                    }
                }
            }
            Verdict::Continue
        });

        for hit in hits {
            out.push(Violation {
                subject: node.name.clone(),
                offender: hit.name,
                rule: hit.reason,
                path: hit.path,
            });
        }
    }

    fn check_layering(
        &self,
        node: &PackageNode,
        rule: &CrateRule,
        graph: &Graph,
        out: &mut Vec<Violation>,
    ) {
        for dep_name in &node.direct_deps {
            // A dependency explicitly named under a `feature.*` list is a
            // deliberate, reviewed tier crossing (e.g. games reaching the
            // client stack only behind their `presentation` feature) — that
            // review IS the gate, so the generic ordering check does not
            // re-litigate it. Only the always-on `allow` set is checked here.
            if rule
                .feature
                .values()
                .any(|deps| deps.iter().any(|d| d == dep_name))
            {
                continue;
            }
            let Some(dep_idx) = graph.workspace_member_index(dep_name) else {
                continue;
            };
            let dep_node = &graph.nodes[dep_idx];
            let Some(dep_rel) = dep_node.rel_path.as_deref() else {
                continue;
            };
            let Some((_, dep_rule)) = self.rule_for(dep_name, dep_rel) else {
                continue;
            };
            if let Some(msg) = self.layering_violation(&node.name, rule, dep_name, dep_rule) {
                out.push(Violation {
                    subject: node.name.clone(),
                    offender: dep_name.clone(),
                    rule: msg,
                    path: Vec::new(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::PackageNode;

    fn matrix() -> Matrix {
        Matrix::parse(
            r#"
            [banned]
            crates = ["tokio"]

            [tiers]
            order = ["deterministic", "contract", "client", "runtime", "binary"]
            unconstrained = ["test", "tooling"]

            [categories.by_name]
            rendering = ["macroquad", "renderer-macroquad"]

            [categories.by_path_prefix]
            application = ["apps", "services"]

            [crate."tabula-core"]
            tier = "deterministic"
            allow = ["serde"]
            forbidden_categories = ["rendering", "application"]

            [crate."tabula-game-api"]
            tier = "deterministic"
            allow = ["tabula-core", "serde"]
            forbidden_categories = ["application"]

            [crate."tabula-presentation"]
            tier = "client"
            allow = ["tabula-core"]

            [crate."renderer-macroquad"]
            tier = "client"
            allow = ["macroquad", "tabula-presentation"]

            [crate."services/tabula-server"]
            tier = "binary"
            allow = ["tabula-core"]
            "#,
        )
        .expect("matrix parses")
    }

    fn node(name: &str, rel_path: Option<&str>, direct_deps: &[&str]) -> PackageNode {
        PackageNode {
            name: name.to_string(),
            rel_path: rel_path.map(str::to_string),
            direct_deps: direct_deps.iter().copied().map(String::from).collect(),
            resolved_deps: Vec::new(),
        }
    }

    fn link(graph: &mut Graph, from: usize, to: usize) {
        graph.nodes[from].resolved_deps.push(to);
    }

    #[test]
    fn valid_dependency_passes() {
        let m = matrix();
        let mut g = Graph {
            nodes: vec![
                node("tabula-core", Some("crates/tabula-core"), &["serde"]),
                node("serde", None, &[]),
            ],
        };
        link(&mut g, 0, 1);
        let violations = m.evaluate(&g);
        assert!(
            violations.is_empty(),
            "expected no violations, got {violations:?}"
        );
    }

    #[test]
    fn core_depending_on_renderer_fails() {
        let m = matrix();
        let mut g = Graph {
            nodes: vec![
                node(
                    "tabula-core",
                    Some("crates/tabula-core"),
                    &["serde", "renderer-macroquad"],
                ),
                node("serde", None, &[]),
                node(
                    "renderer-macroquad",
                    Some("crates/renderer-macroquad"),
                    &["macroquad", "tabula-presentation"],
                ),
                node("macroquad", None, &[]),
                node(
                    "tabula-presentation",
                    Some("crates/tabula-presentation"),
                    &["tabula-core"],
                ),
            ],
        };
        link(&mut g, 0, 1);
        link(&mut g, 0, 2);
        link(&mut g, 2, 3);
        link(&mut g, 2, 4);

        let violations = m.evaluate(&g);
        assert!(
            violations
                .iter()
                .any(|v| v.subject == "tabula-core" && v.offender == "renderer-macroquad"),
            "expected a rendering violation, got {violations:?}"
        );
    }

    #[test]
    fn game_api_depending_on_server_fails() {
        let m = matrix();
        let mut g = Graph {
            nodes: vec![
                node(
                    "tabula-game-api",
                    Some("crates/tabula-game-api"),
                    &["tabula-core", "tabula-server"],
                ),
                node("tabula-core", Some("crates/tabula-core"), &["serde"]),
                node("serde", None, &[]),
                node(
                    "tabula-server",
                    Some("services/tabula-server"),
                    &["tabula-core"],
                ),
            ],
        };
        link(&mut g, 0, 1);
        link(&mut g, 0, 3);
        link(&mut g, 1, 2);
        link(&mut g, 3, 1);

        let violations = m.evaluate(&g);
        assert!(
            violations
                .iter()
                .any(|v| v.subject == "tabula-game-api" && v.offender == "tabula-server"),
            "expected an application-category violation, got {violations:?}"
        );
    }

    #[test]
    fn transitive_banned_crate_is_reported_with_path() {
        let m = matrix();
        let mut g = Graph {
            nodes: vec![
                node("tabula-core", Some("crates/tabula-core"), &["serde"]),
                node("serde", None, &["tokio"]),
                node("tokio", None, &[]),
            ],
        };
        link(&mut g, 0, 1);
        link(&mut g, 1, 2);

        let violations = m.evaluate(&g);
        let hit = violations
            .iter()
            .find(|v| v.offender == "tokio")
            .expect("tokio should be reported as a banned transitive dependency");
        assert_eq!(hit.path, vec!["tabula-core", "serde", "tokio"]);
    }

    #[test]
    fn undeclared_direct_dependency_fails() {
        let m = matrix();
        let mut g = Graph {
            nodes: vec![
                node(
                    "tabula-core",
                    Some("crates/tabula-core"),
                    &["serde", "reqwest"],
                ),
                node("serde", None, &[]),
                node("reqwest", None, &[]),
            ],
        };
        link(&mut g, 0, 1);
        link(&mut g, 0, 2);

        let violations = m.evaluate(&g);
        assert!(violations
            .iter()
            .any(|v| v.subject == "tabula-core" && v.offender == "reqwest"));
    }

    #[test]
    fn layering_violation_when_lower_tier_depends_on_higher_tier() {
        let m = matrix();
        let mut g = Graph {
            nodes: vec![
                node(
                    "tabula-core",
                    Some("crates/tabula-core"),
                    &["tabula-server"],
                ),
                node(
                    "tabula-server",
                    Some("services/tabula-server"),
                    &["tabula-core"],
                ),
            ],
        };
        link(&mut g, 0, 1);
        link(&mut g, 1, 0);

        let violations = m.evaluate(&g);
        assert!(violations.iter().any(|v| v.subject == "tabula-core"
            && v.offender == "tabula-server"
            && v.rule.contains("dependency arrows must point down")));
    }

    #[test]
    fn crate_missing_from_deps_toml_is_reported() {
        let m = matrix();
        let g = Graph {
            nodes: vec![node("tabula-mystery", Some("crates/tabula-mystery"), &[])],
        };
        let violations = m.evaluate(&g);
        assert!(violations.iter().any(|v| v.subject == "tabula-mystery"));
    }
}
