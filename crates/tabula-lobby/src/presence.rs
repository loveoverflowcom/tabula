//! Single-owner attachment bookkeeping (doc 03 §14.2). Authentication and
//! current binding checks remain the storage/gateway's responsibility.

use std::collections::{BTreeMap, BTreeSet};
use tabula_core::UserId;

/// Process-local presence edges, not durable online authority.
#[derive(Default)]
pub struct PresenceTracker {
    users: BTreeMap<UserId, BTreeSet<u64>>,
    connections: BTreeMap<u64, UserId>,
    offline: BTreeMap<UserId, u64>,
    revision: u64,
}
impl std::fmt::Debug for PresenceTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PresenceTracker([REDACTED])")
    }
}

impl PresenceTracker {
    /// Tracks a freshly authorized socket; duplicate attach does not add an edge.
    pub fn attach(&mut self, connection: u64, user: UserId) -> bool {
        if connection == 0 || user.0 == 0 || self.connections.contains_key(&connection) {
            return false;
        }
        self.connections.insert(connection, user);
        let sessions = self.users.entry(user).or_default();
        let transition = sessions.is_empty();
        sessions.insert(connection);
        self.offline.remove(&user);
        if transition {
            self.revision = self.revision.saturating_add(1);
        }
        transition
    }

    /// Only final detach creates an offline transition; another tab remains live.
    pub fn detach(&mut self, connection: u64, now_ms: u64) -> Option<UserId> {
        let user = self.connections.remove(&connection)?;
        let sessions = self.users.get_mut(&user)?;
        sessions.remove(&connection);
        if !sessions.is_empty() {
            return None;
        }
        self.users.remove(&user);
        self.offline.insert(user, now_ms);
        self.revision = self.revision.saturating_add(1);
        Some(user)
    }

    /// Coalescing cursor; intermediate attach/detach edges need no fan-out.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Drains only transitions that remained offline for the five-second debounce.
    pub fn drain_offline(&mut self, now_ms: u64) -> Vec<(UserId, u64)> {
        let ready: Vec<_> = self
            .offline
            .iter()
            .filter_map(|(user, at)| (now_ms.saturating_sub(*at) >= 5_000).then_some((*user, *at)))
            .collect();
        for (user, _) in &ready {
            self.offline.remove(user);
        }
        ready
    }

    /// Retries failed durable metadata without overriding a newer live attachment.
    pub fn retry_offline(&mut self, user: UserId, at_ms: u64) {
        if !self.users.contains_key(&user) {
            self.offline.entry(user).or_insert(at_ms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_socket_detach_is_the_only_offline_edge() {
        let mut owner = PresenceTracker::default();
        assert!(owner.attach(1, UserId(10)));
        assert!(!owner.attach(2, UserId(10)));
        assert_eq!(owner.detach(1, 100), None);
        assert_eq!(owner.detach(2, 200), Some(UserId(10)));
        assert_eq!(owner.drain_offline(5_199), vec![]);
        assert_eq!(owner.drain_offline(5_200), vec![(UserId(10), 200)]);
    }
    #[test]
    fn reconnect_cancels_unconfirmed_offline_transition() {
        let mut owner = PresenceTracker::default();
        owner.attach(1, UserId(10));
        owner.detach(1, 100);
        owner.attach(2, UserId(10));
        assert_eq!(owner.drain_offline(10_000), vec![]);
        assert!(!owner.attach(2, UserId(11)));
        assert_eq!(owner.detach(2, 10_000), Some(UserId(10)));
    }
}
