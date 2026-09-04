//! Pure policy for `xtask check-no-game-ids` (I-9).
//!
//! "No platform crate branches on a `game_id`." (AGENTS.md §2.3) The scanner
//! looks for a game's identifier appearing as a whole token — case-
//! insensitively, and treating `_`/`-` as separators so `tabula_game_chess`
//! and `tabula-game-chess` are caught the same way a bare `"chess"` literal
//! would be — anywhere outside its allowed locations.
//!
//! This cannot distinguish a string literal from a comment from a doc-test:
//! doing that precisely needs a real Rust tokenizer, which is more machinery
//! than a Phase 0 bootstrap check earns. Per the mission brief, the policy is
//! kept deterministic and simple, with a named, extensible allowlist rather
//! than ad hoc exceptions, so it can be sharpened later without changing its
//! shape.

use std::fmt;

/// A line containing this token is exempt, no matter its zone or game id.
/// For the rare case where a word coincides with a game id for an unrelated
/// reason (e.g. `Category::Cards`, a catalog genre — not a specific game
/// package), name it explicitly and visibly in review rather than widening
/// the zone rules to quietly cover it:
///
/// ```rust,ignore
/// Cards, // xtask-allow-game-id: genre, not a specific game package
/// ```
pub const SUPPRESS_MARKER: &str = "xtask-allow-game-id";

/// Where a file sits relative to the game-id policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    /// `games/<id>/**` — may freely name itself, but not another game.
    OwnGamePackage(String),
    /// `crates/tabula-registry/**` — the one crate allowed to name any game
    /// (doc 02 §8, AGENTS.md §2.3, I-9).
    Registry,
    /// `xtask/**` — the tool that defines and enforces this very policy.
    Tooling,
    /// Any path with a `tests/`, `fixtures/`, `testdata/`, or `replays/`
    /// component — explicit test fixtures are exempt by design.
    TestFixture,
    /// `Cargo.toml`, `game.toml`, or any other `*.toml` manifest.
    Manifest,
    /// Any Markdown file, or anything under `docs/`.
    Documentation,
    /// Everything else: platform source that must not name a specific game.
    Restricted,
}

impl Zone {
    pub fn allows(&self, game_id: &str) -> bool {
        match self {
            Zone::OwnGamePackage(owner) => owner.eq_ignore_ascii_case(game_id),
            Zone::Registry
            | Zone::Tooling
            | Zone::TestFixture
            | Zone::Manifest
            | Zone::Documentation => true,
            Zone::Restricted => false,
        }
    }
}

/// Classify a workspace-relative path (posix separators) into a [`Zone`].
pub fn classify_zone(rel_path: &str, game_ids: &[String]) -> Zone {
    let segments: Vec<&str> = rel_path.split('/').collect();

    if segments.first() == Some(&"games") {
        if let Some(pkg) = segments.get(1) {
            if game_ids.iter().any(|g| g == pkg) {
                return Zone::OwnGamePackage((*pkg).to_string());
            }
        }
    }
    if rel_path.starts_with("crates/tabula-registry/") {
        return Zone::Registry;
    }
    if rel_path.starts_with("xtask/") {
        return Zone::Tooling;
    }
    if rel_path.starts_with(".github/") {
        // CI workflow files are declarative configuration, not platform
        // source that could branch on a game id — a self-play matrix or a
        // nightly-job comment naming games is the CI equivalent of a
        // manifest, not an I-9 risk.
        return Zone::Documentation;
    }
    if segments
        .iter()
        .any(|s| matches!(*s, "tests" | "fixtures" | "testdata" | "replays"))
    {
        return Zone::TestFixture;
    }

    let file_name = segments.last().copied().unwrap_or("");
    if file_name == "Cargo.toml" || file_name == "game.toml" || has_extension(file_name, "toml") {
        return Zone::Manifest;
    }
    if has_extension(file_name, "md") || segments.first() == Some(&"docs") {
        return Zone::Documentation;
    }

    Zone::Restricted
}

fn has_extension(file_name: &str, ext: &str) -> bool {
    file_name
        .rsplit_once('.')
        .is_some_and(|(_, actual)| actual.eq_ignore_ascii_case(ext))
}

/// One disallowed occurrence of a game id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub game_id: String,
    pub context: String,
}

impl fmt::Display for Hit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "game-id violation:")?;
        writeln!(f, "  file: {}:{}:{}", self.file, self.line, self.column)?;
        writeln!(f, "  game id: {}", self.game_id)?;
        writeln!(f, "  context: {}", self.context.trim())?;
        write!(
            f,
            "  rule: game identifiers may appear only in their own game package, tabula-registry, xtask, tests/fixtures, manifests, or docs (I-9)"
        )
    }
}

/// Scan one file's contents for disallowed occurrences of any `game_ids`
/// entry, given its workspace-relative path.
///
/// Rust source is riddled with doc comments and rationale comments that
/// legitimately name other games as examples (`//! chess computes elapsed
/// time as ...`) — that is documentation, just inlined rather than filed
/// under `docs/`. Rather than pretending a text scanner can tell a string
/// literal from a doc-test, this treats anything from the first `//` line-
/// comment marker onward as exempt, the same way the `Documentation` zone is.
/// It will not catch a game id smuggled inside a `/* block comment */`; that
/// is a known, accepted gap (see the module doc comment).
pub fn scan_file(rel_path: &str, contents: &str, game_ids: &[String]) -> Vec<Hit> {
    let zone = classify_zone(rel_path, game_ids);
    let is_rust = has_extension(rel_path, "rs");
    let mut hits = Vec::new();

    for (line_idx, line) in contents.lines().enumerate() {
        if line.contains(SUPPRESS_MARKER) {
            continue; // an explicit, reviewed, per-line exception (see const doc)
        }
        let comment_at = if is_rust {
            line_comment_start(line)
        } else {
            None
        };
        for id in game_ids {
            if zone.allows(id) {
                continue;
            }
            for col in find_word_occurrences(line, id) {
                if comment_at.is_some_and(|c| col >= c) {
                    continue; // inside a `//` / `///` / `//!` comment: documentation
                }
                hits.push(Hit {
                    file: rel_path.to_string(),
                    line: line_idx + 1,
                    column: col + 1,
                    game_id: id.clone(),
                    context: line.to_string(),
                });
            }
        }
    }

    hits
}

/// Byte offset of the first `//` or `/*` comment marker in `line`, ignoring
/// a `//` that is part of a `://` URL scheme. Not string-literal-aware: a
/// comment marker inside a string constant is (rarely) misread as a comment
/// start, which only makes the check more permissive, never a false failure.
fn line_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        let is_line_comment =
            bytes[i] == b'/' && bytes[i + 1] == b'/' && (i == 0 || bytes[i - 1] != b':');
        let is_block_comment = bytes[i] == b'/' && bytes[i + 1] == b'*';
        if is_line_comment || is_block_comment {
            return Some(i);
        }
    }
    None
}

/// Byte offsets of every case-insensitive, word-bounded occurrence of
/// `needle` in `haystack`. A boundary is any position that is not flanked on
/// both sides by an ASCII alphanumeric character — `_` and `-` count as
/// boundaries, so `tabula_game_chess` and `tabula-game-chess` match `chess`
/// the same way a bare `"chess"` literal would, while `chessboard` and
/// `postcards` do not.
fn find_word_occurrences(haystack: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    let hay_lower = haystack.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let bytes = hay_lower.as_bytes();
    let nlen = needle_lower.len();

    let is_word = |b: u8| b.is_ascii_alphanumeric();

    let mut hits = Vec::new();
    let mut start = 0;
    while let Some(pos) = hay_lower[start..].find(&needle_lower) {
        let idx = start + pos;
        let before_ok = idx == 0 || !is_word(bytes[idx - 1]);
        let after_idx = idx + nlen;
        let after_ok = after_idx >= bytes.len() || !is_word(bytes[after_idx]);
        if before_ok && after_ok {
            hits.push(idx);
        }
        start = idx + 1;
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<String> {
        ["tictactoe", "chess", "caro", "werewolf", "tiles"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn game_id_in_registry_passes() {
        let hits = scan_file(
            "crates/tabula-registry/src/lib.rs",
            "register!(chess, caro, werewolf);",
            &ids(),
        );
        assert!(hits.is_empty(), "registry may name any game, got {hits:?}");
    }

    #[test]
    fn game_id_in_own_package_passes() {
        let hits = scan_file(
            "games/chess/src/lib.rs",
            "//! tabula-game-chess — the correctness benchmark",
            &ids(),
        );
        assert!(
            hits.is_empty(),
            "a game package may name itself, got {hits:?}"
        );
    }

    #[test]
    fn game_id_in_other_package_fails() {
        let hits = scan_file(
            "games/chess/src/lib.rs",
            "use tabula_game_caro::Mark;",
            &ids(),
        );
        assert!(
            hits.iter().any(|h| h.game_id == "caro"),
            "one game naming another should fail, got {hits:?}"
        );
    }

    #[test]
    fn game_id_hardcoded_in_platform_crate_fails() {
        let hits = scan_file(
            "crates/tabula-lobby/src/room.rs",
            r#"if capabilities.id == "chess" { seats = 2; }"#,
            &ids(),
        );
        assert!(
            hits.iter()
                .any(|h| h.game_id == "chess" && h.file.contains("tabula-lobby")),
            "a hard-coded game id in a platform crate should fail, got {hits:?}"
        );
    }

    #[test]
    fn game_id_as_crate_reference_is_caught() {
        let hits = scan_file(
            "services/tabula-server/src/main.rs",
            "use tabula_game_werewolf::WerewolfModule;",
            &ids(),
        );
        assert!(hits.iter().any(|h| h.game_id == "werewolf"));
    }

    #[test]
    fn substring_false_positives_are_avoided() {
        let hits = scan_file(
            "crates/tabula-core/src/lib.rs",
            "use postcard; // a chessboard is not a game id, and neither are utilities",
            &ids(),
        );
        assert!(
            hits.is_empty(),
            "postcard/chessboard/utilities must not trigger caro/chess/tiles, got {hits:?}"
        );
    }

    #[test]
    fn manifests_and_docs_are_exempt() {
        let toml_hits = scan_file("games/chess/game.toml", "id = \"com.tabula.chess\"", &ids());
        let doc_hits = scan_file(
            "docs/games/chess.md",
            "Chess is the correctness benchmark.",
            &ids(),
        );
        let root_doc_hits = scan_file("AGENTS.md", "cargo xtask new-game chess", &ids());
        assert!(toml_hits.is_empty());
        assert!(doc_hits.is_empty());
        assert!(root_doc_hits.is_empty());
    }

    #[test]
    fn rationale_comments_naming_other_games_are_exempt() {
        let hits = scan_file(
            "crates/tabula-game-api/src/rules.rs",
            "/// Werewolf makes this a core rule; chess never sends it.",
            &ids(),
        );
        assert!(
            hits.is_empty(),
            "doc-comment rationale should be exempt, got {hits:?}"
        );
    }

    #[test]
    fn code_after_a_trailing_comment_is_still_flagged_before_it() {
        let hits = scan_file(
            "crates/tabula-lobby/src/room.rs",
            r#"if id == "chess" { /* werewolf too, eventually */ }"#,
            &ids(),
        );
        assert_eq!(
            hits.iter().map(|h| h.game_id.as_str()).collect::<Vec<_>>(),
            vec!["chess"],
            "the werewolf mention is inside a comment; only the live \"chess\" literal should fail"
        );
    }

    #[test]
    fn ci_workflow_files_are_exempt() {
        let hits = scan_file(
            ".github/workflows/nightly.yml",
            "game: [tictactoe] # add chess (P1), caro/tiles/werewolf (P3)",
            &ids(),
        );
        assert!(
            hits.is_empty(),
            "CI config is declarative, not platform source, got {hits:?}"
        );
    }

    #[test]
    fn explicit_suppression_marker_is_honored() {
        let hits = scan_file(
            "crates/tabula-game-api/src/metadata.rs",
            "    Cards, // xtask-allow-game-id: a catalog genre, not a specific game package",
            &ids(),
        );
        assert!(
            hits.is_empty(),
            "an explicit, reviewed suppression should exempt the line, got {hits:?}"
        );
    }

    #[test]
    fn test_fixtures_are_exempt() {
        let hits = scan_file(
            "tests/replays/chess/golden_001.rs",
            "// fixture for chess replay divergence",
            &ids(),
        );
        assert!(hits.is_empty());
    }
}
