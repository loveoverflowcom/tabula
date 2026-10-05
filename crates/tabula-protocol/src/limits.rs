use core::{fmt, marker::PhantomData};

use serde::de::{self, Deserialize, Deserializer, SeqAccess, Visitor};
use tabula_core::{GameId, GameVersion};

/// Maximum encoded client frame, including codec overhead (doc 05 §9.1).
pub const MAX_INBOUND_FRAME_BYTES: usize = 64 * 1024;
/// Maximum opaque command bytes (doc 05 §9.1).
pub const MAX_GAME_PAYLOAD_BYTES: usize = 16 * 1024;
/// Maximum encoded server frame, including codec overhead (doc 05 §9.1).
pub const MAX_OUTBOUND_FRAME_BYTES: usize = 1024 * 1024;
/// Maximum reverse-DNS game identity in UTF-8 bytes (ADR-0039).
pub const MAX_GAME_ID_BYTES: usize = 128;
/// Maximum game package version in UTF-8 bytes (ADR-0039).
pub const MAX_GAME_VERSION_BYTES: usize = 64;
/// Maximum projected snapshot bytes (ADR-0039).
pub const MAX_VIEW_BYTES: usize = 512 * 1024;
/// Maximum bytes in one redacted event (ADR-0039).
pub const MAX_EVENT_BYTES: usize = 16 * 1024;
/// Maximum redacted events in one observable update (ADR-0039).
pub const MAX_EVENTS: usize = 64;

// Do not trust a format's size_hint or let Vec's default visitor allocate from
// it. Check before allocation and cap incremental growth when no hint exists.
fn bounded_vec<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T, const MAX: usize>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> Visitor<'de> for Bounded<T, MAX> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a sequence of at most {MAX} elements")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let hint = sequence.size_hint().unwrap_or(0);
            if hint > MAX {
                return Err(de::Error::custom("wire collection limit exceeded"));
            }
            let mut values = Vec::with_capacity(hint);
            while let Some(value) = sequence.next_element()? {
                if values.len() == MAX {
                    return Err(de::Error::custom("wire collection limit exceeded"));
                }
                if values.len() == values.capacity() {
                    values.reserve_exact((MAX - values.len()).min(64));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded::<T, MAX>(PhantomData))
}

fn bounded_string<'de, D, const MAX: usize>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct Bounded<const MAX: usize>;
    impl<const MAX: usize> Visitor<'_> for Bounded<MAX> {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a string of at most {MAX} UTF-8 bytes")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<String, E> {
            if value.len() > MAX {
                return Err(E::custom("wire string limit exceeded"));
            }
            Ok(value.to_owned())
        }

        fn visit_string<E: de::Error>(self, value: String) -> Result<String, E> {
            if value.len() > MAX {
                return Err(E::custom("wire string limit exceeded"));
            }
            Ok(value)
        }
    }
    deserializer.deserialize_str(Bounded::<MAX>)
}

pub(crate) fn game_id<'de, D>(deserializer: D) -> Result<GameId, D::Error>
where
    D: Deserializer<'de>,
{
    let value = bounded_string::<D, MAX_GAME_ID_BYTES>(deserializer)?;
    GameId::new(value).map_err(de::Error::custom)
}

pub(crate) fn game_version<'de, D>(deserializer: D) -> Result<GameVersion, D::Error>
where
    D: Deserializer<'de>,
{
    let value = bounded_string::<D, MAX_GAME_VERSION_BYTES>(deserializer)?;
    GameVersion::new(value).map_err(de::Error::custom)
}

pub(crate) fn payload<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_vec::<D, u8, MAX_GAME_PAYLOAD_BYTES>(deserializer)
}

pub(crate) fn view<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_vec::<D, u8, MAX_VIEW_BYTES>(deserializer)
}

pub(crate) fn events<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Event(Vec<u8>);
    impl<'de> Deserialize<'de> for Event {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            bounded_vec::<D, u8, MAX_EVENT_BYTES>(deserializer).map(Self)
        }
    }
    bounded_vec::<D, Event, MAX_EVENTS>(deserializer)
        .map(|events| events.into_iter().map(|event| event.0).collect())
}

pub(crate) fn nonzero<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let counter = u64::deserialize(deserializer)?;
    if counter == 0 {
        return Err(de::Error::custom("wire counter must be nonzero"));
    }
    Ok(counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::{value, DeserializeSeed, SeqAccess};

    struct OversizedHint;
    impl<'de> SeqAccess<'de> for OversizedHint {
        type Error = value::Error;
        fn next_element_seed<T: DeserializeSeed<'de>>(
            &mut self,
            _: T,
        ) -> Result<Option<T::Value>, Self::Error> {
            panic!("oversized hint must be rejected before reading or allocating elements");
        }
        fn size_hint(&self) -> Option<usize> {
            Some(usize::MAX)
        }
    }

    #[test]
    fn hostile_sequence_hints_are_checked_before_allocating_or_reading() {
        assert!(payload(value::SeqAccessDeserializer::new(OversizedHint)).is_err());
        assert!(view(value::SeqAccessDeserializer::new(OversizedHint)).is_err());
        assert!(events(value::SeqAccessDeserializer::new(OversizedHint)).is_err());
    }
}
