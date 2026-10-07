//! Host-owned public identity and avatar resolution. (doc 04 §1, §12; I-10)
//!
//! The host supplies only display facts it is permitted to disclose. This is
//! neither an account/profile API nor an authorization token. No identity,
//! profile request, URL or asset callback belongs in canonical game state.
//! Hosts load an explicitly declared logical asset through their managed
//! resource boundary, then complete the exact pending ticket. Until then all
//! surfaces resolve the same neutral fallback. A different occupant, display
//! revision or host epoch immediately retires the former image and callback.

use std::collections::BTreeMap;

use tabula_core::{SeatId, UserId};
pub use tabula_design::AvatarFallback;
use tabula_game_api::AssetRef;

/// Public occupant reference supplied by the host, never inferred from a seat
/// or role. Local simulator identifiers are guests, not accounts. (doc 00 §13)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicSubject {
    Account(UserId),
    Guest(u128),
    Bot(u128),
    Empty,
}

/// Bounded, explicitly permitted display text; absent profile fields must not
/// be synthesized from account IDs. (doc 04 §1)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicDisplayLabel(String);

impl PublicDisplayLabel {
    /// Rejects blank, control-bearing or overlong text before presentation.
    /// Long permitted names still need adapter truncation and a full label.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()
            && value.chars().count() <= 128
            && !value.chars().any(char::is_control))
        .then_some(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AvatarState {
    Unavailable,
    Loading(AssetRef),
    Ready(AssetRef),
    Failed,
}

/// Host-approved public display facts, shared by shell and gameplay surfaces.
/// Image resolution never depends on a role, phase, seat or death. (I-10)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicDisplay {
    subject: PublicSubject,
    revision: u64,
    label: Option<PublicDisplayLabel>,
    avatar: AvatarState,
}

/// A resolved managed image, or the same neutral fallback on every surface.
/// The image is a logical resource reference, never an arbitrary URL. (doc 04 §12)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicAvatar<'a> {
    Image(&'a AssetRef),
    Fallback(AvatarFallback),
}

impl PublicDisplay {
    /// Creates one approved subject/revision. An image starts pending until the
    /// managed host resource pipeline confirms readiness for its exact ticket.
    #[must_use]
    pub fn new(
        subject: PublicSubject,
        revision: u64,
        label: Option<PublicDisplayLabel>,
        avatar: Option<AssetRef>,
    ) -> Self {
        Self {
            subject,
            revision,
            label,
            avatar: avatar.map_or(AvatarState::Unavailable, AvatarState::Loading),
        }
    }

    #[must_use]
    pub const fn subject(&self) -> PublicSubject {
        self.subject
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_ref().map(PublicDisplayLabel::as_str)
    }

    /// Resolves the appearance from approved identity and resource readiness.
    #[must_use]
    pub fn avatar(&self) -> PublicAvatar<'_> {
        match &self.avatar {
            AvatarState::Ready(asset) => PublicAvatar::Image(asset),
            AvatarState::Unavailable | AvatarState::Loading(_) | AvatarState::Failed => {
                PublicAvatar::Fallback(match self.subject {
                    PublicSubject::Account(_) | PublicSubject::Guest(_) => AvatarFallback::Human,
                    PublicSubject::Bot(_) => AvatarFallback::Bot,
                    PublicSubject::Empty => AvatarFallback::Empty,
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    generation: u64,
    display: PublicDisplay,
}

/// Exact pending resource request, scoped to the host epoch and seat binding.
/// A late completion cannot attach the previous occupant's image. (doc 04 §12)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarRequest {
    epoch: u64,
    generation: u64,
    seat: SeatId,
    subject: PublicSubject,
    revision: u64,
    asset: AssetRef,
}

impl AvatarRequest {
    #[must_use]
    pub const fn subject(&self) -> PublicSubject {
        self.subject
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Declared logical asset to resolve through the host's resource pipeline.
    #[must_use]
    pub const fn asset(&self) -> &AssetRef {
        &self.asset
    }
}

/// Why a host cannot establish a new public seat binding. (doc 04 §1)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicDisplayMapError {
    GenerationExhausted,
}

/// Host-owned seat-to-public-display input, separate from rules and projections.
/// Each new host/session activation must use a different epoch; cloned maps
/// are snapshots within that epoch, not independent callback owners. (I-10)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicDisplayMap {
    epoch: u64,
    next_generation: u64,
    entries: BTreeMap<SeatId, Binding>,
}

impl PublicDisplayMap {
    #[must_use]
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            next_generation: 0,
            entries: BTreeMap::new(),
        }
    }

    /// Replaces a seat atomically, immediately dropping all former image state.
    /// Rebinding the same subject also retires an earlier pending callback.
    pub fn set(
        &mut self,
        seat: SeatId,
        display: PublicDisplay,
    ) -> Result<(), PublicDisplayMapError> {
        let next = self
            .next_generation
            .checked_add(1)
            .ok_or(PublicDisplayMapError::GenerationExhausted)?;
        self.entries.insert(
            seat,
            Binding {
                generation: self.next_generation,
                display,
            },
        );
        self.next_generation = next;
        Ok(())
    }

    /// Vacating a seat drops both its public facts and any pending image owner.
    pub fn remove(&mut self, seat: SeatId) {
        self.entries.remove(&seat);
    }

    #[must_use]
    pub fn get(&self, seat: SeatId) -> Option<&PublicDisplay> {
        self.entries.get(&seat).map(|binding| &binding.display)
    }

    /// Captures the exact currently pending image request for the managed host.
    #[must_use]
    pub fn avatar_request(&self, seat: SeatId) -> Option<AvatarRequest> {
        let binding = self.entries.get(&seat)?;
        let AvatarState::Loading(asset) = &binding.display.avatar else {
            return None;
        };
        Some(AvatarRequest {
            epoch: self.epoch,
            generation: binding.generation,
            seat,
            subject: binding.display.subject,
            revision: binding.display.revision,
            asset: asset.clone(),
        })
    }

    /// Confirms readiness or failure only for the exact active pending request.
    /// The shell owns loading/verification; `true` is its readiness fact, not a
    /// claim that this pure resolver fetched or verified any bytes itself.
    pub fn complete_avatar(&mut self, request: &AvatarRequest, ready: bool) -> bool {
        if request.epoch != self.epoch {
            return false;
        }
        let Some(binding) = self.entries.get_mut(&request.seat) else {
            return false;
        };
        if binding.generation != request.generation
            || binding.display.subject != request.subject
            || binding.display.revision != request.revision
            || binding.display.avatar != AvatarState::Loading(request.asset.clone())
        {
            return false;
        }
        binding.display.avatar = if ready {
            AvatarState::Ready(request.asset.clone())
        } else {
            AvatarState::Failed
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: u128, revision: u64, asset: bool) -> PublicDisplay {
        PublicDisplay::new(
            PublicSubject::Account(UserId(id)),
            revision,
            None,
            asset.then(|| AssetRef::new("account/avatar").unwrap()),
        )
    }

    #[test]
    fn absent_loading_failed_and_offline_share_the_human_fallback() {
        let mut map = PublicDisplayMap::new(1);
        map.set(SeatId(0), display(10, 1, false)).unwrap();
        let fallback = PublicAvatar::Fallback(AvatarFallback::Human);
        assert_eq!(map.get(SeatId(0)).unwrap().avatar(), fallback);
        map.set(SeatId(0), display(10, 2, true)).unwrap();
        assert_eq!(map.get(SeatId(0)).unwrap().avatar(), fallback);
        let pending = map.avatar_request(SeatId(0)).unwrap();
        assert!(map.complete_avatar(&pending, false));
        assert_eq!(map.get(SeatId(0)).unwrap().avatar(), fallback);
        assert!(!map.complete_avatar(&pending, true));
        assert_eq!(map.get(SeatId(0)).unwrap().avatar(), fallback);
    }

    #[test]
    fn moving_an_account_between_seats_preserves_the_approved_appearance() {
        let mut map = PublicDisplayMap::new(1);
        for seat in [SeatId(1), SeatId(9)] {
            map.set(seat, display(42, 7, true)).unwrap();
            let request = map.avatar_request(seat).unwrap();
            assert!(map.complete_avatar(&request, true));
        }
        assert_eq!(map.get(SeatId(1)), map.get(SeatId(9)));
        assert_eq!(
            map.get(SeatId(9)).unwrap().avatar(),
            PublicAvatar::Image(&AssetRef::new("account/avatar").unwrap())
        );
    }

    #[test]
    fn occupant_and_revision_changes_retire_images_and_late_callbacks() {
        let mut map = PublicDisplayMap::new(1);
        let seat = SeatId(3);
        map.set(seat, display(10, 1, true)).unwrap();
        let old = map.avatar_request(seat).unwrap();
        assert!(map.complete_avatar(&old, true));
        map.set(seat, display(20, 1, true)).unwrap();
        assert_eq!(
            map.get(seat).unwrap().avatar(),
            PublicAvatar::Fallback(AvatarFallback::Human)
        );
        assert!(!map.complete_avatar(&old, true));
        let next = map.avatar_request(seat).unwrap();
        map.set(seat, display(20, 2, true)).unwrap();
        assert!(!map.complete_avatar(&next, true));
        let current = map.avatar_request(seat).unwrap();
        assert!(map.complete_avatar(&current, true));
    }

    #[test]
    fn vacate_same_subject_rebind_and_new_host_epoch_reject_old_tickets() {
        let seat = SeatId(3);
        let mut map = PublicDisplayMap::new(1);
        map.set(seat, display(10, 1, true)).unwrap();
        let old = map.avatar_request(seat).unwrap();
        map.remove(seat);
        assert!(map.get(seat).is_none());
        assert!(!map.complete_avatar(&old, true));
        map.set(seat, display(10, 1, true)).unwrap();
        assert!(!map.complete_avatar(&old, true));
        let mut replacement = PublicDisplayMap::new(2);
        replacement.set(seat, display(10, 1, true)).unwrap();
        assert!(!replacement.complete_avatar(&old, true));
    }

    #[test]
    fn guest_bot_and_empty_markers_do_not_depend_on_subject_numbers() {
        for (subject, expected) in [
            (PublicSubject::Account(UserId(1)), AvatarFallback::Human),
            (
                PublicSubject::Account(UserId(u128::MAX)),
                AvatarFallback::Human,
            ),
            (PublicSubject::Guest(1), AvatarFallback::Human),
            (PublicSubject::Guest(900), AvatarFallback::Human),
            (PublicSubject::Bot(1), AvatarFallback::Bot),
            (PublicSubject::Bot(99), AvatarFallback::Bot),
            (PublicSubject::Empty, AvatarFallback::Empty),
        ] {
            assert_eq!(
                PublicDisplay::new(subject, 0, None, None).avatar(),
                PublicAvatar::Fallback(expected)
            );
        }
    }

    #[test]
    fn explicit_labels_are_bounded_without_synthesizing_profile_data() {
        assert!(PublicDisplayLabel::new(" ").is_none());
        assert!(PublicDisplayLabel::new("Alice\nBob").is_none());
        assert!(PublicDisplayLabel::new("A".repeat(129)).is_none());
        assert!(PublicDisplayLabel::new("Đ".repeat(128)).is_some());
        assert_eq!(
            PublicDisplayLabel::new("Minh Anh").unwrap().as_str(),
            "Minh Anh"
        );
        assert_eq!(display(42, 0, false).label(), None);
    }

    #[test]
    fn exhausted_generation_rejects_without_changing_the_current_binding() {
        let mut map = PublicDisplayMap::new(1);
        map.set(SeatId(0), display(10, 1, false)).unwrap();
        map.next_generation = u64::MAX;
        let before = map.clone();
        assert_eq!(
            map.set(SeatId(0), display(20, 1, false)),
            Err(PublicDisplayMapError::GenerationExhausted)
        );
        assert_eq!(map, before);
    }
}
