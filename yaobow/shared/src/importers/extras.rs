//! Parsing for the `asset.extras.yaobow` (and per-node `extras.yaobow`)
//! metadata block emitted by [`crate::exporters::gltf`] (see
//! `GlbBuilder::set_yaobow_extras`), so a round-tripped glTF can recover
//! the source format's reserved/"unknown" fields that have no natural
//! glTF representation.
//!
//! The exporter always writes `{"yaobow": {"schema": 1, "payload": {...}}}`
//! at `asset.extras`; `payload`'s shape is target-format specific (see the
//! `target_format` key) and is interpreted by each converter in
//! [`crate::importers::mv3`], [`crate::importers::pol`], and
//! [`crate::importers::cvd`] — this module only extracts the envelope.

use serde_json::Value;

/// Parsed `asset.extras.yaobow` envelope.
#[derive(Debug, Clone)]
pub struct YaobowExtras {
    /// `yaobow.schema`. Only schema `1` (the only version ever emitted) is
    /// understood; higher schemas are accepted but their `payload` is
    /// treated as opaque (callers should ignore fields they don't
    /// recognize rather than failing the import).
    pub schema: u64,
    /// `yaobow.payload`, target-format specific.
    pub payload: Value,
}

impl YaobowExtras {
    /// `payload.target_format`, e.g. `"mv3"` / `"pol"` / `"cvd"`, used to
    /// sanity-check that the extras actually describe the format being
    /// converted to (mismatches are surfaced as a warning, not an error,
    /// since a user may deliberately be retargeting an edited asset).
    pub fn target_format(&self) -> Option<&str> {
        self.payload.get("target_format").and_then(Value::as_str)
    }
}

/// Parses a raw glTF `extras` JSON blob (as produced by `serde_json`'s
/// `RawValue`) looking for the `yaobow` envelope described above. Returns
/// `None` (not an error) if `extras` is absent, isn't an object, or has no
/// `yaobow` key — plain/hand-authored glTF simply has no round-trip
/// metadata to recover.
pub fn parse_yaobow_extras(extras: Option<&str>) -> Option<YaobowExtras> {
    let raw = extras?;
    let value: Value = serde_json::from_str(raw).ok()?;
    let envelope = value.get("yaobow")?;
    let schema = envelope.get("schema").and_then(Value::as_u64).unwrap_or(1);
    let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
    Some(YaobowExtras { schema, payload })
}
