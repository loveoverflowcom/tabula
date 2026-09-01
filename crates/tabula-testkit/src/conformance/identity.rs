//! Stable game identity. (doc 02 §4.1)
//!
//! Checked directly against `GameModule::metadata().id` — the one place a
//! `GameId` is authoritative for a compiled game. The registry naming policy
//! (`id` must equal `com.tabula.<game directory name>`, doc 02 §4.1) is
//! enforced exactly, against `game.toml`, by `xtask check-manifests` — that
//! check knows the directory name and this one does not. What this check
//! verifies is the weaker, code-only invariant every consumer of
//! `GameModule::metadata()` actually relies on: the id is non-empty, shaped
//! like the reverse-DNS scheme the whole registry uses, and stable across
//! calls (it must be a compile-time constant, not something that varies).

use tabula_game_api::GameModule;

use super::support;
use super::GameTestFixture;

pub fn check<F: GameTestFixture>() {
    let id_a = F::Module::metadata().id().clone();
    let id_b = F::Module::metadata().id().clone();

    assert!(
        !id_a.as_str().is_empty(),
        "{}",
        support::failure(
            "stable game identity",
            "<empty>",
            "GameMetadata::id must not be empty."
        )
    );

    assert!(
        is_reverse_dns(id_a.as_str()),
        "{}",
        support::failure(
            "stable game identity",
            id_a.as_str(),
            &format!(
                "GameId must be a reverse-DNS identifier of lowercase alphanumeric \
                 segments separated by '.', e.g. \"com.tabula.example\" (doc 02 §4.1). \
                 Got: {:?}",
                id_a.as_str()
            )
        )
    );

    assert_eq!(
        id_a,
        id_b,
        "{}",
        support::failure(
            "stable game identity",
            id_a.as_str(),
            "GameMetadata::id returned two different values across calls; a game's \
             identity must be a compile-time constant, not something computed anew."
        )
    );
}

/// `segment(.segment)+`, each segment lowercase-alphanumeric and starting
/// with a letter. Deliberately permissive about length and segment count —
/// the exact `com.tabula.<slug>` policy is xtask's job (see module docs);
/// this only guards the shape every consumer of a `GameId` assumes holds.
fn is_reverse_dns(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|seg| {
            seg.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::is_reverse_dns;

    #[test]
    fn accepts_the_documented_shape() {
        assert!(is_reverse_dns("com.tabula.example"));
    }

    #[test]
    fn rejects_empty_and_malformed_ids() {
        assert!(!is_reverse_dns(""));
        assert!(!is_reverse_dns("example"));
        assert!(!is_reverse_dns("Com.Tabula.Example"));
        assert!(!is_reverse_dns("com..example"));
        assert!(!is_reverse_dns(".com.tabula"));
    }
}
