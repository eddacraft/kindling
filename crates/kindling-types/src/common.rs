use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

/// Unique identifier for entities. Implementation uses UUIDv4 format.
pub type Id = String;

/// Timestamp in epoch milliseconds.
///
/// Aliased to `i64` to keep arithmetic ergonomic in Rust. Every public field
/// that holds a `Timestamp` MUST carry a `#[cfg_attr(feature = "ts-rs",
/// ts(type = "number"))]` override (use `ts(optional, type = "number")` for
/// `Option<Timestamp>`); without it the `ts-rs` projection would emit
/// `bigint`, breaking the JSON `number` wire contract. Round-trip and
/// bindings tests will fail if the override is missed.
pub type Timestamp = i64;

/// Scope identifiers for multi-dimensional isolation.
///
/// All fields are optional to support partial scoping. Mirrors `ScopeIds`
/// in `packages/kindling-core/src/types/common.ts`.
///
/// On the wire this is always a keyed JSON object; arrays are rejected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ScopeIds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub repo_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub task_id: Option<String>,
}

/// Map-only deserialization shape for [`ScopeIds`]. Keep its fields in step
/// with `ScopeIds`; the destructuring in `From` fails to compile if one is
/// added there and not here.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScopeIdsFields {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    repo_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
}

impl From<ScopeIdsFields> for ScopeIds {
    fn from(f: ScopeIdsFields) -> Self {
        let ScopeIdsFields {
            session_id,
            repo_id,
            agent_id,
            user_id,
            task_id,
        } = f;
        ScopeIds {
            session_id,
            repo_id,
            agent_id,
            user_id,
            task_id,
        }
    }
}

// Written by hand because serde's derived impl also accepts a sequence and
// binds its elements to fields by position, so `["x"]` would silently become
// `{ "sessionId": "x" }` (GH-216). Only a map is accepted here.
impl<'de> Deserialize<'de> for ScopeIds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapOnly;

        impl<'de> Visitor<'de> for MapOnly {
            type Value = ScopeIds;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a scopeIds object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ScopeIds, A::Error> {
                ScopeIdsFields::deserialize(MapAccessDeserializer::new(map)).map(ScopeIds::from)
            }
        }

        deserializer.deserialize_map(MapOnly)
    }
}

/// Validation error details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS), ts(export, export_to = "../bindings/"))]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub value: Option<serde_json::Value>,
}
