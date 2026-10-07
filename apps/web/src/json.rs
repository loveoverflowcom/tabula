//! Shared strict response lexer for the browser DTO boundary (ADR-0044).
//!
//! Each response is an object with unique decoded keys at every depth. DTO
//! deserialization still owns fields and numeric types; it establishes no
//! authority. One lexer avoids specializing byte parsing for every DTO.

use serde::{de, Deserialize, Deserializer};
use serde_json::{Map, Number, Value};

/// Decode a bounded response object without normalizing duplicate fields away.
pub(crate) fn decode<T: de::DeserializeOwned>(bytes: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_value(object(bytes)?)
}

#[inline(never)]
fn object(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    // from_slice retains Serde JSON's depth limit and rejects trailing input.
    let StrictValue(value) = serde_json::from_slice::<StrictValue>(bytes)?;
    if !value.is_object() {
        return Err(de::Error::custom("response must be an object"));
    }
    Ok(value)
}

struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> de::Visitor<'de> for StrictVisitor {
    type Value = StrictValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with unique object keys")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(Number::from(value))))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Number(Number::from(value))))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Number::from_f64(value)
            .map(|number| StrictValue(Value::Number(number)))
            .ok_or_else(|| E::custom("nonfinite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::String(value.to_owned())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::String(value)))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictValue(Value::Null))
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(StrictValue(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(StrictValue(Value::Array(values)))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut fields: A) -> Result<Self::Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = fields.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate object key"));
            }
            let StrictValue(value) = fields.next_value()?;
            values.insert(key, value);
        }
        Ok(StrictValue(Value::Object(values)))
    }
}

#[cfg(test)]
mod tests {
    use super::decode;
    use tabula_session_http::accounts::SelfAccountProfileResponse;
    use tabula_session_http::PublicProblem;

    fn profile(revision: &str, as_of_ms: &str) -> String {
        format!(
            r#"{{"version":2,"account_id":"00000000000000000000000000000001","handle":"valid_handle","display_name":"Đặng 東京","visibility":"private","revision":{revision},"as_of_ms":{as_of_ms}}}"#
        )
    }

    #[test]
    fn strict_lexer_rejects_decoded_duplicate_keys_at_every_depth() {
        for input in [
            r#"{"version":1,"version":2,"status":409,"title":"Conflict","code":"revision_conflict"}"#,
            r#"{"version":1,"\u0076ersion":2,"status":409,"title":"Conflict","code":"revision_conflict"}"#,
            r#"{"outer":[{"field":1,"field":2}]}"#,
            r#"{"outer":{"field":1,"\u0066ield":2}}"#,
        ] {
            assert!(decode::<serde_json::Value>(input.as_bytes()).is_err());
        }
    }

    #[test]
    fn strict_object_boundary_rejects_positional_trailing_and_deep_inputs() {
        for input in [
            r#"[1,409,"Conflict","revision_conflict"]"#,
            r#"{"version":1,"status":409,"title":"Conflict","code":"revision_conflict"} {}"#,
            r#"{"version":1,"status":409,"title":"Conflict","code":"revision_conflict""#,
        ] {
            assert!(decode::<PublicProblem>(input.as_bytes()).is_err());
        }
        let deep = format!("{{\"nested\":{}{}}}", "[".repeat(130), "]".repeat(130));
        assert!(decode::<serde_json::Value>(deep.as_bytes()).is_err());
    }

    #[test]
    fn shared_lexer_keeps_u64_exact_and_rejects_noninteger_dto_fields() {
        let exact = decode::<SelfAccountProfileResponse>(
            profile("9007199254740993", "18446744073709551615").as_bytes(),
        )
        .expect("integer DTO");
        assert_eq!(exact.revision, 9_007_199_254_740_993);
        assert_eq!(exact.as_of_ms, u64::MAX);
        for number in ["1.0", "1e0", "-1", "18446744073709551616"] {
            assert!(decode::<SelfAccountProfileResponse>(profile(number, "1").as_bytes()).is_err());
        }
    }

    #[test]
    fn dto_unknown_fields_and_version_validation_remain_required() {
        let valid = profile("1", "1");
        let decoded = decode::<SelfAccountProfileResponse>(valid.as_bytes()).expect("valid DTO");
        assert!(decoded.validate().is_ok());
        let unknown = valid.replace("\"revision\":1", "\"revision\":1,\"unexpected\":true");
        assert!(decode::<SelfAccountProfileResponse>(unknown.as_bytes()).is_err());
        let wrong = valid.replace("\"version\":2", "\"version\":3");
        let decoded = decode::<SelfAccountProfileResponse>(wrong.as_bytes())
            .expect("shape alone has no authority");
        assert!(decoded.validate().is_err());
    }
}
