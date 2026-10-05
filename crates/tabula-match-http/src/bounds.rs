//! Allocation-aware JSON/serde field bounds (doc05 §9.1; ADR-0041).
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{collections::BTreeMap, fmt, marker::PhantomData};
use tabula_protocol::ServerEnvelope;
struct Text<const MAX: usize>(String);
impl<'de, const MAX: usize> Deserialize<'de> for Text<MAX> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Bounded<const MAX: usize>;
        impl<const MAX: usize> Visitor<'_> for Bounded<MAX> {
            type Value = Text<MAX>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "text of at most {MAX} bytes")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.len() > MAX {
                    Err(E::custom("text exceeds field limit"))
                } else {
                    Ok(Text(v.into()))
                }
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                if v.len() > MAX {
                    Err(E::custom("text exceeds field limit"))
                } else {
                    Ok(Text(v))
                }
            }
        }
        d.deserialize_string(Bounded::<MAX>)
    }
}
pub(crate) fn text<'de, D: Deserializer<'de>, const MAX: usize>(d: D) -> Result<String, D::Error> {
    Text::<MAX>::deserialize(d).map(|v| v.0)
}
pub(crate) fn token<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    text::<D, 2048>(d)
}
pub(crate) fn id<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    text::<D, 32>(d)
}
pub(crate) fn game<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    text::<D, 128>(d)
}
pub(crate) fn short<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    text::<D, 64>(d)
}
pub(crate) fn optional_token<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<Text<2048>>::deserialize(d).map(|v| v.map(|v| v.0))
}
pub(crate) fn optional_code<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<Text<12>>::deserialize(d).map(|v| v.map(|v| v.0))
}
pub(crate) fn config<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct Config;
    impl<'de> Visitor<'de> for Config {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most sixteen bounded unique config fields")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            if map.size_hint().is_some_and(|n| n > 16) {
                return Err(de::Error::custom("too many config fields"));
            }
            let mut values = BTreeMap::new();
            while let Some(key) = map.next_key::<Text<64>>()? {
                if values.len() >= 16 {
                    return Err(de::Error::custom("too many config fields"));
                }
                let value = map.next_value::<Text<128>>()?;
                if values.insert(key.0, value.0).is_some() {
                    return Err(de::Error::custom("duplicate config field"));
                }
            }
            Ok(values)
        }
    }
    d.deserialize_map(Config)
}
pub(crate) fn frames<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ServerEnvelope>, D::Error> {
    struct Frames(PhantomData<ServerEnvelope>);
    impl<'de> Visitor<'de> for Frames {
        type Value = Vec<ServerEnvelope>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most sixteen bounded projected frames")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            if seq.size_hint().is_some_and(|n| n > 16) {
                return Err(de::Error::custom("too many frames"));
            }
            let mut frames = Vec::new();
            while let Some(frame) = seq.next_element::<ServerEnvelope>()? {
                if frames.len() >= 16 {
                    return Err(de::Error::custom("too many frames"));
                }
                frames.push(frame);
            }
            Ok(frames)
        }
    }
    d.deserialize_seq(Frames(PhantomData))
}
