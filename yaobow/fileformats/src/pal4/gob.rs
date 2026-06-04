use std::io::SeekFrom;

use binrw::{BinRead, BinResult};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::utils::{parse_sized_string, SizedString};

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobFile {
    pub header: GobHeader,

    #[br(count = header.count)]
    pub entries: Vec<GobEntry>,
}

/// Per-entry object kind tag stored in [`GobHeader::object_types`].
///
/// Engine logic historically only branched on `ITEM` (0) and `EFFECT` (8),
/// but in practice each tag value strongly correlates with the contents of
/// the entry's parameter block (see `generated/gob2.md` §1.1):
///
/// | Tag | Role                                              |
/// |----:|----------------------------------------------------|
/// |  0  | Generic / scenario item (may carry "examine" cb)  |
/// |  3  | Sound emitter (dummy `MC` mesh)                   |
/// |  5  | State-machine driven prop (switch/lever)          |
/// |  6  | Animated prop (door, banner, fire, …)             |
/// |  7  | Pickup / treasure item                            |
/// |  8  | Particle / effect placeholder (dummy `ZZA` mesh)  |
/// |  9  | Script-only marker / "flag" (dummy `JDumy` mesh)  |
pub struct GobObjectType;
impl GobObjectType {
    pub const GENERIC: u32 = 0;
    pub const SOUND: u32 = 3;
    pub const MACHINE: u32 = 5;
    pub const ACTION: u32 = 6;
    pub const GET_ITEM: u32 = 7;
    pub const EFFECT: u32 = 8;
    pub const MARKER: u32 = 9;

    /// Legacy alias kept for source compatibility with earlier code that
    /// branched on `ITEM` vs `EFFECT`. Prefer the more specific constants
    /// above for new code.
    pub const ITEM: u32 = Self::GENERIC;

    /// Returns a short, human-readable name for an observed tag value,
    /// or `None` for any value never seen in the corpus.
    pub fn name(value: u32) -> Option<&'static str> {
        match value {
            Self::GENERIC => Some("generic"),
            Self::SOUND => Some("sound"),
            Self::MACHINE => Some("machine"),
            Self::ACTION => Some("action"),
            Self::GET_ITEM => Some("get_item"),
            Self::EFFECT => Some("effect"),
            Self::MARKER => Some("marker"),
            _ => None,
        }
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobHeader {
    pub count: u32,

    #[br(count = count)]
    pub object_types: Vec<u32>,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobEntry {
    pub name: SizedString,
    pub folder: SizedString,
    pub file_name: SizedString,
    /// Top-level RenderWare atomic name inside [`file_name`]. The current
    /// consumer (`asset_loader::load_object`) only addresses meshes by the
    /// outer file name, so this field is exposed for completeness but not
    /// yet used to pick a sub-atomic.
    pub file_name2: SizedString,
    pub position: [f32; 3],
    /// Euler angles in degrees, applied intrinsically as Z → Y → X.
    pub rotation: [f32; 3],
    /// Script function (`gas`) invoked on the player's "Examine" /
    /// "Research" action. Empty string means no examine handler.
    pub research_function: SizedString,

    /// Three boolean flags. Best-guess names (see gob2.md §2.1):
    /// `[0]` = `scripted` (entry has scripted side-effects),
    /// `[1]` = `save_tracked` (state persists into save game),
    /// `[2]` = `interactive` (surfaces a research/use panel).
    /// All entries observed only ever use `0` or `1`.
    pub flags: [u32; 3],

    /// Trigger / culling distance in world units (best guess, gob2.md §2.2).
    /// Sound emitters use ~600.0; everything else defaults to ~60.0.
    pub trigger_distance: f32,

    /// Initial "active" flag. Always `1` in the shipped corpus, so `0` is
    /// treated as "disabled at load" by analogy.
    pub active: u32,

    pub game_object: GobPropertyI32,

    #[br(count = game_object.value)]
    pub properties: Vec<GobProperty>,

    pub parameters_begin: GobPropertyI32,

    #[br(parse_with = parse_properties)]
    pub parameters: Vec<GobProperty>,
}

/// Names of the generic, region-1 ("fixed") object-level properties. See
/// `generated/gob2.md` §5.2 for the full catalogue and observed counts.
pub struct GobFixedProperty;
impl GobFixedProperty {
    pub const AUTO_DISAPPEAR: &'static str = "PAL4-GameObject-object-auto-disappear";
    pub const RESEARCH_NUM: &'static str = "PAL4-GameObject-object-research-num";
    pub const STORE: &'static str = "PAL4-GameObject-object-store";
    pub const HEIGHT_LIMIT: &'static str = "PAL4_GameObject-object-height-limit";
    pub const CLIP: &'static str = "PAL4-GameObject-object-clip";
    pub const CAMERA_COLLIDE: &'static str = "PAL4-GameObject-object-camera-collide";
    pub const MARKER: &'static str = "PAL4-GameObject-object-marker";
    pub const SCALE: &'static str = "PAL4-GameObject-object-scale";
    pub const HIDE: &'static str = "PAL4-GameObject-object-hide";
    pub const VALIDFOR: &'static str = "PAL4-GameObject-object-validfor";
    pub const HINT: &'static str = "PAL4-GameObject-object-hint";
    pub const ID: &'static str = "PAL4-GameObject-object-id";
}

/// Block-marker names that delimit each parameter sub-system in region 2.
pub struct GobParameterBlock;
impl GobParameterBlock {
    pub const EFFECT: &'static str = "PAL4_GameObject-effect";
    pub const ACTION: &'static str = "PAL4_GameObject-action";
    pub const GET_ITEM: &'static str = "PAL4_GameObject-getItem";
    pub const MACHINE: &'static str = "PAL4_GameObject-machine";
    pub const SOUND: &'static str = "PAL4-GameObject-sound";
}

/// Effect (tag 8) parameter names.
pub struct GobEffectProperty;
impl GobEffectProperty {
    pub const NAME: &'static str = "PAL4_GameObject-effect-name";
    pub const TIMES: &'static str = "PAL4_GameObject-effect-times";
    pub const SAVER: &'static str = "PAL4_GameObject-effect-saver";
    pub const DISTANCE_CLIP: &'static str = "PAL4_GameObject-effect-distance-clip";
}

/// Action / animated-prop (tag 6) parameter names.
pub struct GobActionProperty;
impl GobActionProperty {
    pub const DEFAULT_PLAY: &'static str = "PAL4_GameObject-action-default-play";
    pub const HOLDING_END: &'static str = "PAL4_GameObject-action-holding-end";
    pub const PLAY_TIMES: &'static str = "PAL4_GameObject-action-play-times";
}

/// GetItem / pickup (tag 7) parameter names.
pub struct GobGetItemProperty;
impl GobGetItemProperty {
    pub const EQUIP_ID: &'static str = "PAL4_GameObject-getItem-EquipID";
    pub const PROP_ID: &'static str = "PAL4_GameObject-getItem-PropID";
    pub const NUM: &'static str = "PAL4_GameObject-getItem-num";
}

/// Machine (tag 5) parameter names.
pub struct GobMachineProperty;
impl GobMachineProperty {
    pub const REPEAT: &'static str = "PAL4_GameObject-machine-repeat";
    pub const AUTO_TOUCH: &'static str = "PAL4_GameObject-machine-auto-touch";
    pub const CONDITION: &'static str = "PAL4_GameObject-machine-condition";
    pub const PROPID: &'static str = "PAL4_GameObject-machine-propid";
    pub const PROPNUM: &'static str = "PAL4_GameObject-machine-propnum";

    /// Prefix shared by all task-list arrays: `PAL4-GOMTask-[ N ]`.
    pub const TASK_LIST_PREFIX: &'static str = "PAL4-GOMTask-[";
}

/// Sound emitter (tag 3) parameter names.
pub struct GobSoundProperty;
impl GobSoundProperty {
    pub const NAME: &'static str = "PAL4-GameObject-sound-name";
    pub const MIN_TIME: &'static str = "PAL4-GameObject-sound-mintime";
    pub const MAX_TIME: &'static str = "PAL4-GameObject-sound-maxtime";
}

/// Names of the per-task sub-properties found inside each
/// `PAL4-GOMTask-[ N ]` array item (gob2.md §5.4).
pub struct GobGomTaskProperty;
impl GobGomTaskProperty {
    pub const TYPE: &'static str = "PAL4_GOMTask_type";
    pub const WAIT_MODE: &'static str = "PAL4_GOMTask_wait_mode";
    pub const WAIT_TIME: &'static str = "PAL4_GOMTask_wait_time";
    pub const EXPT_TIME: &'static str = "PAL4_GOMTask_expt_time";
    pub const ACTION_NAME: &'static str = "PAL4_GOMTask_action_name";
    pub const ACTION_TIME: &'static str = "PAL4_GOMTask_action_time";
    pub const GETITEM_ID: &'static str = "PAL4_GOMTask_getitem_id";
    pub const GETITEM_NUM: &'static str = "PAL4_GOMTask_getitem_num";
    pub const CAMERA_QUAKE_AMPLITUDE: &'static str = "PAL4_GOMTask_camera_quake_amplitude";
    pub const CAMERA_QUAKE_FREQUENCY: &'static str = "PAL4_GOMTask_camera_quake_frequency";
    pub const CAMERA_QUAKE_TIME: &'static str = "PAL4_GOMTask_camera_quake_time";
    pub const ROTATE_TARGET_X: &'static str = "PAL4_GOMTask_rotate_target_X";
    pub const ROTATE_TARGET_Y: &'static str = "PAL4_GOMTask_rotate_target_Y";
    pub const ROTATE_TARGET_Z: &'static str = "PAL4_GOMTask_rotate_target_Z";
    pub const ROTATE_EFFECT_NAME: &'static str = "PAL4_GOMTask_rotate_effect_name";
    pub const ROTATE_EFFECT_TIME: &'static str = "PAL4_GOMTask_rotate_effect_time";
    pub const ROTATE_SOUND_NAME: &'static str = "PAL4_GOMTask_rotate_sound_name";
    pub const ROTATE_SOUND_TIME: &'static str = "PAL4_GOMTask_rotate_sound_time";
    pub const ACTIVE_OBJECT_CTRL: &'static str = "PAL4_GOMTask_active_object_ctrl";
    pub const ACTIVE_OBJECT_NAME: &'static str = "PAL4_GOMTask_active_object_name";
    pub const TIME_WAIT_MINVAL: &'static str = "PAL4_GOMTask_time_wait_minval";
    pub const TIME_WAIT_MAXVAL: &'static str = "PAL4_GOMTask_time_wait_maxval";
}

/// Numerical opcode stored as `PAL4_GOMTask_type`. The only observed
/// values are 1, 3, 7, 8, 13, 14, 15, 19, 20, 21 — kept as an opaque
/// `i32` newtype so unknown ids round-trip cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GomTaskType(pub i32);

/// Legacy enum kept for source compatibility with the small set of
/// callers that already use `get_common_property`. New code should
/// prefer the `GobFixedProperty::*` string constants and
/// [`GobEntry::get_property`] directly.
pub enum GobCommonProperties {
    Scale,
    ResearchNum,
    AutoDisappear,
}

/// Legacy enum kept for source compatibility (see
/// [`GobCommonProperties`]).
pub enum GobCommonParameters {
    EffectName,
    EffectTimes,
}

impl GobEntry {
    pub fn get_property(&self, name: &str) -> Option<&GobProperty> {
        self.properties
            .iter()
            .find_map(|p| if p.name() == name { Some(p) } else { None })
    }

    pub fn get_parameter(&self, name: &str) -> Option<&GobProperty> {
        self.parameters
            .iter()
            .find_map(|p| if name == p.name() { Some(p) } else { None })
    }

    pub fn get_common_property(&self, property: GobCommonProperties) -> Option<&GobProperty> {
        match property {
            GobCommonProperties::Scale => self.get_property(GobFixedProperty::SCALE),
            GobCommonProperties::ResearchNum => self.get_property(GobFixedProperty::RESEARCH_NUM),
            GobCommonProperties::AutoDisappear => {
                self.get_property(GobFixedProperty::AUTO_DISAPPEAR)
            }
        }
    }

    pub fn get_common_parameter(&self, parameter: GobCommonParameters) -> Option<&GobProperty> {
        match parameter {
            GobCommonParameters::EffectName => self.get_parameter(GobEffectProperty::NAME),
            GobCommonParameters::EffectTimes => self.get_parameter(GobEffectProperty::TIMES),
        }
    }

    /// Best-guess: entry is referenced by gameplay script. See gob2.md §2.1.
    pub fn is_scripted(&self) -> bool {
        self.flags[0] != 0
    }

    /// Best-guess: entry state persists into the save game. See gob2.md §2.1.
    pub fn is_save_tracked(&self) -> bool {
        self.flags[1] != 0
    }

    /// Best-guess: entry surfaces a research/use panel on interact.
    /// See gob2.md §2.1.
    pub fn is_interactive(&self) -> bool {
        self.flags[2] != 0
    }

    /// `false` only if [`active`](Self::active) is explicitly `0`.
    /// All shipped entries set it to `1`.
    pub fn is_active_on_load(&self) -> bool {
        self.active != 0
    }

    /// Convenience accessor for `PAL4-GameObject-object-hide`. Returns
    /// `true` only if the property is present and set to a non-zero
    /// value.
    pub fn is_initially_hidden(&self) -> bool {
        self.get_property(GobFixedProperty::HIDE)
            .and_then(|p| p.value_i32())
            .map(|v| v != 0)
            .unwrap_or(false)
    }

    /// Iterate over the GOMTask arrays attached to this entry. Each
    /// item is a `(level, array)` pair, where `level` is the `N` in
    /// `PAL4-GOMTask-[ N ]` (`0` if it cannot be parsed).
    pub fn gom_task_lists(&self) -> impl Iterator<Item = (u32, &GobPropertyObjectArray)> {
        self.parameters.iter().filter_map(|p| match p {
            GobProperty::GobPropertyObjectArray(arr) => {
                let name = p.name();
                if name.starts_with(GobMachineProperty::TASK_LIST_PREFIX) {
                    let level = name
                        .trim_start_matches(GobMachineProperty::TASK_LIST_PREFIX)
                        .trim()
                        .trim_end_matches(']')
                        .trim()
                        .parse::<u32>()
                        .unwrap_or(0);
                    Some((level, arr))
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    /// Convenience accessor for `PAL4-GameObject-sound-name` (SOUND tag,
    /// see [`GobSoundProperty::NAME`]). Returns the raw `.wav` stem
    /// (without extension), or `None` if absent / non-string / empty.
    pub fn sound_name(&self) -> Option<String> {
        let s = self.get_parameter(GobSoundProperty::NAME)?.value_string()?;
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    }

    /// Convenience accessor for `PAL4-GameObject-sound-mintime` in
    /// seconds. Returns `None` if absent or non-f32. Caller is
    /// responsible for clamping NaN / negative / near-zero values.
    pub fn sound_min_time(&self) -> Option<f32> {
        self.get_parameter(GobSoundProperty::MIN_TIME)?.value_f32()
    }

    /// Convenience accessor for `PAL4-GameObject-sound-maxtime` in
    /// seconds. Returns `None` if absent or non-f32. Caller is
    /// responsible for clamping NaN / negative / `max < min`.
    pub fn sound_max_time(&self) -> Option<f32> {
        self.get_parameter(GobSoundProperty::MAX_TIME)?.value_f32()
    }
}

#[binrw::parser(reader, endian)]
fn parse_properties() -> BinResult<Vec<GobProperty>> {
    let mut properties = vec![];

    loop {
        let ty = match reader.read_u32_le() {
            Ok(v) => v,
            // Tolerate a clean EOF here: 10/271 corpus files end exactly on
            // the last entry's parameter terminator without writing the
            // closing `u32 = 0`. See gob2.md §8.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if ty == 0 {
            break;
        } else {
            reader.seek(SeekFrom::Current(-4))?;
        }

        let property = GobProperty::read_options(reader, endian, ())?;
        properties.push(property);
    }

    Ok(properties)
}

#[derive(Debug, Serialize)]
pub enum GobProperty {
    GobPropertyI32(GobPropertyI32),
    GobPropertyF32(GobPropertyF32),
    GobPropertyString(GobPropertyString),
    GobPropertyObjectArray(GobPropertyObjectArray),
}

impl BinRead for GobProperty {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let start_position = reader.stream_position()?;
        let ty = reader.read_u32_le()?;
        let name = SizedString::read_options(reader, endian, ())?;
        reader.seek(SeekFrom::Start(start_position))?;

        if name == "PAL4_GameObject-machine-condition"
            || name
                .data()
                .starts_with(GobMachineProperty::TASK_LIST_PREFIX.as_bytes())
        {
            return Ok(Self::GobPropertyObjectArray(
                GobPropertyObjectArray::read_options(reader, endian, ())?,
            ));
        } else {
            match ty {
                1 => Ok(Self::GobPropertyI32(GobPropertyI32::read_options(
                    reader,
                    endian,
                    (),
                )?)),
                2 => Ok(Self::GobPropertyF32(GobPropertyF32::read_options(
                    reader,
                    endian,
                    (),
                )?)),
                3 => Ok(Self::GobPropertyString(GobPropertyString::read_options(
                    reader,
                    endian,
                    (),
                )?)),
                _ => {
                    unreachable!(
                        "Unknown array name: {:?} at position {}",
                        name.to_string(),
                        start_position
                    );
                }
            }
        }
    }
}

impl GobProperty {
    pub fn name(&self) -> &str {
        match self {
            Self::GobPropertyI32(v) => &v.name,
            Self::GobPropertyF32(v) => &v.name,
            Self::GobPropertyString(v) => &v.name,
            Self::GobPropertyObjectArray(v) => &v.0[0].name,
        }
    }

    pub fn value_i32(&self) -> Option<i32> {
        if let Self::GobPropertyI32(v) = self {
            Some(v.value)
        } else {
            None
        }
    }

    pub fn value_f32(&self) -> Option<f32> {
        if let Self::GobPropertyF32(v) = self {
            Some(v.value)
        } else {
            None
        }
    }

    pub fn value_string(&self) -> Option<&str> {
        if let Self::GobPropertyString(v) = self {
            Some(&v.value)
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GobPropertyObjectArray(pub Vec<GobObject>);

impl BinRead for GobPropertyObjectArray {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut properties = vec![];

        let count = reader.read_u32_le()?;
        reader.seek(SeekFrom::Current(-4))?;
        for _ in 0..count {
            // First iteration: the `u32` we consume here is the array
            // count itself (already read above). Subsequent iterations:
            // an inter-item padding `u32` (observed always `0`). Either
            // way we discard it before reading the next item.
            let _ = reader.read_u32_le()?;

            let obj = GobObject::read_options(reader, endian, ())?;
            properties.push(obj);
        }

        Ok(Self(properties))
    }
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyI32 {
    pub ty: u32,

    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub value: i32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyF32 {
    pub ty: u32,

    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub value: f32,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobPropertyString {
    pub ty: u32,

    #[br(parse_with = parse_sized_string)]
    pub name: String,

    #[br(parse_with = parse_sized_string)]
    pub value: String,
}

#[derive(Debug, BinRead, Serialize)]
#[brw(little)]
pub struct GobObject {
    #[br(parse_with = parse_sized_string)]
    pub name: String,
    pub prop_count: u32,

    #[br(count = prop_count)]
    pub properties: Vec<GobProperty>,
}

impl GobObject {
    pub fn get_property(&self, name: &str) -> Option<&GobProperty> {
        self.properties.iter().find(|p| p.name() == name)
    }

    /// Returns the `PAL4_GOMTask_type` value for this record (only
    /// meaningful for objects inside a `PAL4-GOMTask-[ N ]` array).
    pub fn gom_task_type(&self) -> Option<GomTaskType> {
        self.get_property(GobGomTaskProperty::TYPE)
            .and_then(|p| p.value_i32())
            .map(GomTaskType)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    /// Sentinel: lock the three GOB tag values that
    /// `Pal4Scene::load` skips rendering for (non-visual entries
    /// whose mesh field is always a placeholder per
    /// `tools/pal4_gob_inspect`). A drift on any of these would
    /// silently re-enable hundreds of unwanted entities — pin them
    /// so a future renumbering / reordering edit fails the test
    /// instead of regressing the runtime.
    #[test]
    fn non_visual_tag_constants_are_pinned() {
        assert_eq!(GobObjectType::SOUND, 3);
        assert_eq!(GobObjectType::EFFECT, 8);
        assert_eq!(GobObjectType::MARKER, 9);
    }

    /// Helper: emit a `SizedString` (u32 length + UTF-8/GBK bytes).
    fn write_string(buf: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        buf.write_all(&(bytes.len() as u32).to_le_bytes()).unwrap();
        buf.write_all(bytes).unwrap();
    }

    fn write_u32(buf: &mut Vec<u8>, v: u32) {
        buf.write_all(&v.to_le_bytes()).unwrap();
    }

    fn write_i32(buf: &mut Vec<u8>, v: i32) {
        buf.write_all(&v.to_le_bytes()).unwrap();
    }

    fn write_f32(buf: &mut Vec<u8>, v: f32) {
        buf.write_all(&v.to_le_bytes()).unwrap();
    }

    /// Property: `ty=1` (i32) + name + value.
    fn write_i32_prop(buf: &mut Vec<u8>, name: &str, value: i32) {
        write_u32(buf, 1);
        write_string(buf, name);
        write_i32(buf, value);
    }

    /// Property: `ty=2` (f32) + name + value.
    #[allow(dead_code)]
    fn write_f32_prop(buf: &mut Vec<u8>, name: &str, value: f32) {
        write_u32(buf, 2);
        write_string(buf, name);
        write_f32(buf, value);
    }

    /// Property: `ty=3` (string) + name + value.
    fn write_str_prop(buf: &mut Vec<u8>, name: &str, value: &str) {
        write_u32(buf, 3);
        write_string(buf, name);
        write_string(buf, value);
    }

    /// Common entry header used by every test fixture (matches the
    /// worked example in gob2.md §6, with one trailing block of
    /// `properties` and `parameters` left for callers to append).
    fn write_entry_header(
        buf: &mut Vec<u8>,
        name: &str,
        folder: &str,
        file_name: &str,
        file_name2: &str,
        property_count: i32,
    ) {
        write_string(buf, name);
        write_string(buf, folder);
        write_string(buf, file_name);
        write_string(buf, file_name2);
        // position
        write_f32(buf, 0.0);
        write_f32(buf, 0.0);
        write_f32(buf, 0.0);
        // rotation
        write_f32(buf, 0.0);
        write_f32(buf, 0.0);
        write_f32(buf, 0.0);
        // research_function
        write_string(buf, "");
        // flags
        write_u32(buf, 0);
        write_u32(buf, 0);
        write_u32(buf, 0);
        // trigger_distance
        write_f32(buf, 60.0);
        // active
        write_u32(buf, 1);
        // game_object section marker
        write_i32_prop(buf, "PAL4-GameObject", property_count);
    }

    /// Single-entry effect file (tag 8), mirroring gob2.md §6.
    #[test]
    fn parses_synthetic_effect_entry() {
        let mut buf = Vec::<u8>::new();
        // header: count=1, object_types=[8]
        write_u32(&mut buf, 1);
        write_u32(&mut buf, GobObjectType::EFFECT);

        // entry
        write_entry_header(
            &mut buf,
            "Jeffect001",
            "gamedata\\PALObject\\MC\\",
            "MC",
            "MC1",
            2,
        );
        // properties (region 1)
        write_i32_prop(&mut buf, GobFixedProperty::HIDE, 0);
        write_i32_prop(&mut buf, GobFixedProperty::STORE, 1);
        // parameters_begin
        write_i32_prop(&mut buf, "PAL4-GameObject-Parameters", 0);
        // parameters (region 2)
        write_i32_prop(&mut buf, GobParameterBlock::EFFECT, 3);
        write_str_prop(&mut buf, GobEffectProperty::NAME, "H_081");
        write_i32_prop(&mut buf, GobEffectProperty::SAVER, 0);
        write_i32_prop(&mut buf, GobEffectProperty::TIMES, -1);
        // terminator
        write_u32(&mut buf, 0);

        let mut cursor = Cursor::new(buf);
        let gob = GobFile::read(&mut cursor).expect("parse synthetic effect");

        assert_eq!(gob.header.count, 1);
        assert_eq!(gob.header.object_types, vec![GobObjectType::EFFECT]);
        let entry = &gob.entries[0];
        assert_eq!(entry.name, "Jeffect001");
        assert_eq!(entry.file_name2, "MC1");
        assert_eq!(entry.trigger_distance, 60.0);
        assert!(entry.is_active_on_load());
        assert!(!entry.is_initially_hidden());
        assert_eq!(
            entry
                .get_parameter(GobEffectProperty::NAME)
                .and_then(|p| p.value_string()),
            Some("H_081"),
        );
    }

    /// Locks in the `PAL4-GOMTask-[ N ]` prefix matching (`[ 2 ]` used
    /// to crash in `unreachable!()`).
    #[test]
    fn parses_gom_task_with_nonzero_level() {
        let mut buf = Vec::<u8>::new();
        write_u32(&mut buf, 1);
        write_u32(&mut buf, GobObjectType::MACHINE);

        write_entry_header(
            &mut buf,
            "item101",
            "gamedata\\PALObject\\OM07\\",
            "OM07",
            "xb",
            1,
        );
        write_i32_prop(&mut buf, GobFixedProperty::HIDE, 0);
        write_i32_prop(&mut buf, "PAL4-GameObject-Parameters", 0);
        write_i32_prop(&mut buf, GobParameterBlock::MACHINE, 1);

        // PAL4-GOMTask-[ 2 ] array with 1 record holding 1 property
        // (`PAL4_GOMTask_type = 7`).
        let arr_name = "PAL4-GOMTask-[ 2 ]";
        write_u32(&mut buf, 1); // count
        write_string(&mut buf, arr_name);
        write_u32(&mut buf, 1); // prop_count for record 0
        write_i32_prop(&mut buf, GobGomTaskProperty::TYPE, 7);

        write_u32(&mut buf, 0); // parameter terminator

        let mut cursor = Cursor::new(buf);
        let gob = GobFile::read(&mut cursor).expect("parse synthetic machine");

        let entry = &gob.entries[0];
        let tasks: Vec<_> = entry.gom_task_lists().collect();
        assert_eq!(tasks.len(), 1, "should see one task list");
        let (level, arr) = tasks[0];
        assert_eq!(level, 2);
        assert_eq!(arr.0.len(), 1);
        assert_eq!(arr.0[0].gom_task_type(), Some(GomTaskType(7)));
    }

    /// Locks in EOF tolerance at the trailing parameter terminator
    /// (gob2.md §8): 10/271 files end the last entry without writing
    /// the closing `u32 = 0`.
    #[test]
    fn parses_entry_without_trailing_terminator() {
        let mut buf = Vec::<u8>::new();
        write_u32(&mut buf, 1);
        write_u32(&mut buf, GobObjectType::MARKER);

        write_entry_header(
            &mut buf,
            "marker001",
            "gamedata\\PALObject\\JDumy\\",
            "JDumy",
            "jg",
            1,
        );
        write_i32_prop(&mut buf, GobFixedProperty::HIDE, 0);
        write_i32_prop(&mut buf, "PAL4-GameObject-Parameters", 0);
        // NOTE: no terminator and no parameters — EOF marks end-of-region.

        let mut cursor = Cursor::new(buf);
        let gob = GobFile::read(&mut cursor).expect("EOF terminator must be tolerated");
        assert_eq!(gob.entries.len(), 1);
        assert!(gob.entries[0].parameters.is_empty());
    }

    /// SOUND tag (3): convenience accessors round-trip the
    /// `(name, mintime, maxtime)` triple used by the ambient-emitter
    /// driver. Empty / missing fields yield `None` so callers can
    /// skip the entry without falling back on placeholder data.
    #[test]
    fn sound_accessors_round_trip() {
        let mut buf = Vec::<u8>::new();
        write_u32(&mut buf, 2);
        write_u32(&mut buf, GobObjectType::SOUND);
        write_u32(&mut buf, GobObjectType::SOUND);

        // Entry 0: well-formed SOUND emitter.
        write_entry_header(
            &mut buf,
            "sound001",
            "gamedata\\PALObject\\MC\\",
            "MC",
            "mc",
            1,
        );
        write_i32_prop(&mut buf, GobFixedProperty::HIDE, 0);
        write_i32_prop(&mut buf, "PAL4-GameObject-Parameters", 0);
        write_i32_prop(&mut buf, GobParameterBlock::SOUND, 3);
        write_str_prop(&mut buf, GobSoundProperty::NAME, "WA01");
        write_f32_prop(&mut buf, GobSoundProperty::MIN_TIME, 8.5);
        write_f32_prop(&mut buf, GobSoundProperty::MAX_TIME, 24.0);
        write_u32(&mut buf, 0);

        // Entry 1: missing NAME (empty string) — accessor must
        // return None so the load-time emitter walk skips it
        // rather than scheduling a `play_sound("")` call.
        write_entry_header(
            &mut buf,
            "sound002",
            "gamedata\\PALObject\\MC\\",
            "MC",
            "mc",
            1,
        );
        write_i32_prop(&mut buf, GobFixedProperty::HIDE, 0);
        write_i32_prop(&mut buf, "PAL4-GameObject-Parameters", 0);
        write_i32_prop(&mut buf, GobParameterBlock::SOUND, 1);
        write_str_prop(&mut buf, GobSoundProperty::NAME, "");
        write_u32(&mut buf, 0);

        let mut cursor = Cursor::new(buf);
        let gob = GobFile::read(&mut cursor).expect("parse synthetic sound entries");
        assert_eq!(gob.entries.len(), 2);

        let e0 = &gob.entries[0];
        assert_eq!(e0.sound_name().as_deref(), Some("WA01"));
        assert_eq!(e0.sound_min_time(), Some(8.5));
        assert_eq!(e0.sound_max_time(), Some(24.0));

        let e1 = &gob.entries[1];
        assert_eq!(e1.sound_name(), None);
        assert_eq!(e1.sound_min_time(), None);
        assert_eq!(e1.sound_max_time(), None);
    }

    /// Developer-local round-trip against a real install of the game.
    /// Ignored by default — run explicitly with `--ignored`.
    #[test]
    #[ignore]
    fn test_gob_real_file() {
        use std::fs::File;
        use std::io::BufReader;

        let file =
            File::open("F:\\PAL4\\gamedata\\scenedata\\scenedata\\M01\\1\\GameObjs.gob").unwrap();
        let mut reader = BufReader::new(file);
        let gob_file = GobFile::read(&mut reader).unwrap();
        println!("{:#?}", gob_file);
    }
}
