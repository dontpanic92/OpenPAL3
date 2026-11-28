use std::io::{self, ErrorKind};

const MAGIC: &[u8] = b"__PAL(Softstar_Ltd: Cool_J)_GameDataBase_OutFile__";
const ROLE_SIZE: usize = 852;
const ITEM_SIZE: usize = 800;
const MAGIC_SIZE: usize = 760;

#[derive(Debug, Default, Clone)]
pub struct RoleDefinition {
    pub id: u32,
    pub kind: u8,
    pub description: String,
    pub model: String,
    pub image: String,
    pub name: String,
    pub level: i32,
    /// HP/Qi/MP/attack/defense/speed/luck, then water/fire/wind/thunder/earth.
    pub attributes: [i32; 12],
    pub attack_elements: [i32; 5],
    pub experience: i32,
    pub normal_attack: u32,
    pub spells: [u32; 4],
    pub spell_levels: [u8; 4],
    pub qi_gain: i32,
    pub money: i16,
}

/// Native 0x478fd0 indexes 852-byte records; 0x4ed47d initializes enemy stats.
/// This block also contains the authored party-role definitions.
pub fn read_roles(bytes: &[u8]) -> io::Result<Vec<RoleDefinition>> {
    let invalid = || io::Error::new(ErrorKind::InvalidData, "Invalid PAL3 GDB role block");
    if bytes.len() < 0xa0 || !bytes.starts_with(MAGIC) {
        return Err(invalid());
    }
    let offset = u32::from_le_bytes(bytes[0x80..0x84].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[0x84..0x88].try_into().unwrap()) as usize;
    if count == 0 && offset == 0 {
        return Ok(vec![]);
    }
    let end = offset
        .checked_add(count.checked_mul(ROLE_SIZE).ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    if offset < 0xa0 {
        return Err(invalid());
    }
    let records = bytes.get(offset..end).ok_or_else(invalid)?;
    let text = |bytes: &[u8]| {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        crate::utils::to_gbk_string(&bytes[..end]).map_err(|_| invalid())
    };
    records
        .chunks_exact(ROLE_SIZE)
        .map(|record| {
            let integer =
                |offset| i32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
            Ok(RoleDefinition {
                id: u32::from_le_bytes(record[..4].try_into().unwrap()),
                kind: record[4],
                description: text(&record[6..0x206])?,
                model: text(&record[0x206..0x224])?,
                image: text(&record[0x224..0x244])?,
                name: text(&record[0x258..0x278])?,
                level: integer(0x278),
                attributes: std::array::from_fn(|i| integer(0x27c + i * 4)),
                attack_elements: std::array::from_fn(|i| integer(0x244 + i * 4)),
                experience: integer(0x2e4),
                normal_attack: integer(0x2ec) as u32,
                spells: std::array::from_fn(|i| integer(0x2f8 + i * 4) as u32),
                spell_levels: record[0x308..0x30c].try_into().unwrap(),
                qi_gain: integer(0x30c),
                // Native settlement 0x4fb3f0 sign-extends this field.
                money: i16::from_le_bytes(record[0x352..0x354].try_into().unwrap()),
            })
        })
        .collect()
}

#[derive(Debug, Default, Clone)]
pub struct MagicDefinition {
    pub id: u32,
    pub kind: u8,
    /// Water/fire/wind/thunder/earth, encoded as five native 32-bit flags.
    pub elements: [bool; 5],
    pub name: String,
    pub description: String,
    /// Unlike the item block, these flags are already in gameplay role order.
    pub usable_by: [bool; 5],
    /// Cumulative uses required for the three proficiency increases.
    pub proficiency_uses: [u8; 3],
    /// Skill unlock level for kind 0; successor unlock level for elemental magic.
    pub learning_level: u8,
    /// One-based proficiency required to learn `successor`.
    pub successor_rank: u8,
    pub successor: u32,
    pub mixed_recipes: [MagicRecipe; 3],
}

#[derive(Debug, Default, Clone)]
pub struct MagicRecipe {
    pub spell: u32,
    pub other_spell: u32,
    pub source_rank: u8,
    pub other_rank: u8,
    pub character_level: u8,
}

pub fn read_magic(bytes: &[u8]) -> io::Result<Vec<MagicDefinition>> {
    let invalid = || io::Error::new(ErrorKind::InvalidData, "Invalid PAL3 GDB magic block");
    if bytes.len() < 0xa0 || !bytes.starts_with(MAGIC) {
        return Err(invalid());
    }
    let offset = u32::from_le_bytes(bytes[0x88..0x8c].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[0x8c..0x90].try_into().unwrap()) as usize;
    if count == 0 && offset == 0 {
        return Ok(vec![]);
    }
    let end = offset
        .checked_add(count.checked_mul(MAGIC_SIZE).ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    if offset < 0xa0 {
        return Err(invalid());
    }
    let records = bytes.get(offset..end).ok_or_else(invalid)?;
    let text = |bytes: &[u8]| {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        crate::utils::to_gbk_string(&bytes[..end]).map_err(|_| invalid())
    };
    records
        .chunks_exact(MAGIC_SIZE)
        .map(|record| {
            Ok(MagicDefinition {
                id: u32::from_le_bytes(record[..4].try_into().unwrap()),
                kind: record[4],
                elements: std::array::from_fn(|i| {
                    u32::from_le_bytes(record[8 + i * 4..12 + i * 4].try_into().unwrap()) == 1
                }),
                name: text(&record[0x1c..0x3c])?,
                description: text(&record[0x3c..0x23c])?,
                usable_by: std::array::from_fn(|i| record[0x23c + i] != 0),
                proficiency_uses: record[0x2c6..0x2c9].try_into().unwrap(),
                learning_level: record[0x2c9],
                successor_rank: record[0x2ca],
                successor: u32::from_le_bytes(record[0x2cc..0x2d0].try_into().unwrap()),
                mixed_recipes: std::array::from_fn(|i| MagicRecipe {
                    spell: u32::from_le_bytes(
                        record[0x2d4 + i * 4..0x2d8 + i * 4].try_into().unwrap(),
                    ),
                    other_spell: u32::from_le_bytes(
                        record[0x2e0 + i * 4..0x2e4 + i * 4].try_into().unwrap(),
                    ),
                    source_rank: record[0x2ef + i],
                    other_rank: record[0x2ec + i],
                    character_level: record[0x2f2 + i],
                }),
            })
        })
        .collect()
}

#[derive(Debug, Default, Clone)]
pub struct ItemDefinition {
    pub id: u32,
    pub name: String,
    pub model: String,
    pub image: String,
    pub description: String,
    pub price: i32,
    pub category: u8,
    pub subtype: u8,
    pub usable_by: [bool; 5],
    pub use_kind: u8,
    /// 1: single character; 2: party/world.
    pub use_scope: u8,
    /// Bit 0: battle; bit 1: adventure.
    pub use_context: u8,
    /// Native condition effects, starting with wind/thunder/water/fire/earth poison.
    pub conditions: [i16; 31],
    pub modes: [u8; 12],
    pub amounts: [i16; 12],
    pub elements: [bool; 5],
}

/// Read common metadata and stat effects from the compiled database's item block.
pub fn read_items(bytes: &[u8]) -> io::Result<Vec<ItemDefinition>> {
    let invalid = || io::Error::new(ErrorKind::InvalidData, "Invalid PAL3 GDB item block");
    if bytes.len() < 0xa0 || !bytes.starts_with(MAGIC) {
        return Err(invalid());
    }

    let offset = u32::from_le_bytes(bytes[0x90..0x94].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[0x94..0x98].try_into().unwrap()) as usize;
    let size = count.checked_mul(ITEM_SIZE).ok_or_else(invalid)?;
    let end = offset.checked_add(size).ok_or_else(invalid)?;
    if offset < 0xa0 {
        return Err(invalid());
    }
    let records = bytes.get(offset..end).ok_or_else(invalid)?;
    let text = |bytes: &[u8]| {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        crate::utils::to_gbk_string(&bytes[..end]).map_err(|_| invalid())
    };
    records
        .chunks_exact(ITEM_SIZE)
        .map(|record| {
            Ok(ItemDefinition {
                id: u32::from_le_bytes(record[..4].try_into().unwrap()),
                name: text(&record[4..0x24])?,
                model: text(&record[0x24..0x42])?,
                image: text(&record[0x42..0x60])?,
                description: text(&record[0x60..0x260])?,
                price: i32::from_le_bytes(record[0x260..0x264].try_into().unwrap()),
                category: record[0x264],
                subtype: record[0x265],
                // Disk order puts Changqing before Longkui and Zixuan.
                usable_by: [0x266, 0x267, 0x269, 0x26a, 0x268].map(|i| record[i] != 0),
                use_kind: record[0x274],
                use_scope: record[0x275],
                use_context: record[0x276],
                conditions: std::array::from_fn(|i| {
                    i16::from_le_bytes(record[0x2bc + i * 2..0x2be + i * 2].try_into().unwrap())
                }),
                modes: record[0x277..0x283].try_into().unwrap(),
                amounts: std::array::from_fn(|i| {
                    i16::from_le_bytes(record[0x284 + i * 2..0x286 + i * 2].try_into().unwrap())
                }),
                elements: std::array::from_fn(|i| record[0x26b + i] != 0),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_role_combat_metadata_and_signed_values_in_native_order() {
        let mut bytes = vec![0; 0xa0 + ROLE_SIZE];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[0x80..0x84].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes[0x84..0x88].copy_from_slice(&1u32.to_le_bytes());
        let record = &mut bytes[0xa0..];
        record[..4].copy_from_slice(&701u32.to_le_bytes());
        record[4] = 1;
        record[6..10].copy_from_slice(b"desc");
        record[0x206..0x209].copy_from_slice(b"B01");
        record[0x224] = b'1';
        record[0x258..0x25c].copy_from_slice(&[0xb2, 0xe2, 0xca, 0xd4]);
        record[0x278..0x27c].copy_from_slice(&2i32.to_le_bytes());
        let attributes = [350i32, 0, 26, 60, 8, 3, 10, -1, 1, 2, 3, 4];
        for (i, value) in attributes.iter().enumerate() {
            record[0x27c + i * 4..0x280 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        for (i, value) in [5i32, 4, 3, 2, 1].iter().enumerate() {
            record[0x244 + i * 4..0x248 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        record[0x2e4..0x2e8].copy_from_slice(&36i32.to_le_bytes());
        record[0x2ec..0x2f0].copy_from_slice(&924u32.to_le_bytes());
        for (i, value) in [210u32, 206, 0, 0].iter().enumerate() {
            record[0x2f8 + i * 4..0x2fc + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        record[0x308..0x30c].copy_from_slice(&[1, 3, 0, 0]);
        record[0x30c..0x310].copy_from_slice(&8i32.to_le_bytes());
        record[0x350..0x352].copy_from_slice(&123i16.to_le_bytes());
        record[0x352..0x354].copy_from_slice(&(-30i16).to_le_bytes());
        let roles = read_roles(&bytes).unwrap();
        let role = &roles[0];
        assert_eq!((role.id, role.kind, role.level), (701, 1, 2));
        assert_eq!(role.name, "测试");
        assert_eq!(
            (&*role.description, &*role.model, &*role.image),
            ("desc", "B01", "1")
        );
        assert_eq!(role.attributes, attributes);
        assert_eq!(role.attack_elements, [5, 4, 3, 2, 1]);
        assert_eq!(
            (
                role.experience,
                role.normal_attack,
                role.qi_gain,
                role.money
            ),
            (36, 924, 8, -30)
        );
        assert_eq!(role.spells, [210, 206, 0, 0]);
        assert_eq!(role.spell_levels, [1, 3, 0, 0]);
    }

    #[test]
    fn rejects_invalid_role_blocks_and_accepts_absent_fixture_data() {
        let mut bytes = vec![0; 0xa0];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        assert!(read_roles(&bytes).unwrap().is_empty());
        bytes[0x84..0x88].copy_from_slice(&1u32.to_le_bytes());
        assert!(read_roles(&bytes).is_err());
        bytes[0x80..0x84].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes.resize(0xa0 + ROLE_SIZE - 1, 0);
        assert!(read_roles(&bytes).is_err());
        bytes[0x80..0x84].copy_from_slice(&u32::MAX.to_le_bytes());
        bytes[0x84..0x88].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_roles(&bytes).is_err());
        bytes[0] = 0;
        assert!(read_roles(&bytes).is_err());
        assert!(read_roles(&[]).is_err());
    }

    #[test]
    fn reads_magic_records_without_reordering_role_or_element_flags() {
        let mut bytes = vec![0; 0xa0 + MAGIC_SIZE * 2];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[0x88..0x8c].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes[0x8c..0x90].copy_from_slice(&2u32.to_le_bytes());
        let record = &mut bytes[0xa0..0xa0 + MAGIC_SIZE];
        record[..4].copy_from_slice(&301u32.to_le_bytes());
        record[4] = 2;
        record[8..12].copy_from_slice(&2u32.to_le_bytes());
        record[16..20].copy_from_slice(&1u32.to_le_bytes());
        record[0x1c..0x20].copy_from_slice(&[0xb2, 0xe2, 0xca, 0xd4]);
        record[0x3c..0x41].copy_from_slice(b"$desc");
        record[0x23c..0x241].copy_from_slice(&[1, 0, 1, 0, 0]);
        record[0x2c6..0x2cb].copy_from_slice(&[15, 30, 45, 4, 2]);
        record[0x2cc..0x2d0].copy_from_slice(&302u32.to_le_bytes());
        record[0x2d4..0x2d8].copy_from_slice(&326u32.to_le_bytes());
        record[0x2e0..0x2e4].copy_from_slice(&319u32.to_le_bytes());
        record[0x2ec] = 2;
        record[0x2ef] = 3;
        record[0x2f2] = 35;
        bytes[0xa0 + MAGIC_SIZE..0xa4 + MAGIC_SIZE].copy_from_slice(&4101u32.to_le_bytes());
        let spells = read_magic(&bytes).unwrap();
        assert_eq!(spells.len(), 2);
        assert_eq!(spells[0].id, 301);
        assert_eq!(spells[0].kind, 2);
        assert_eq!(spells[0].name, "\u{6d4b}\u{8bd5}");
        assert_eq!(spells[0].description, "$desc");
        assert_eq!(spells[0].elements, [false, false, true, false, false]);
        assert_eq!(spells[0].usable_by, [true, false, true, false, false]);
        assert_eq!(spells[0].proficiency_uses, [15, 30, 45]);
        assert_eq!(
            (
                spells[0].learning_level,
                spells[0].successor_rank,
                spells[0].successor
            ),
            (4, 2, 302)
        );
        let recipe = &spells[0].mixed_recipes[0];
        assert_eq!(
            (
                recipe.spell,
                recipe.other_spell,
                recipe.source_rank,
                recipe.other_rank,
                recipe.character_level
            ),
            (326, 319, 3, 2, 35)
        );
        assert_eq!(spells[1].id, 4101);
        assert_eq!(spells[1].kind, 0);
    }

    #[test]
    fn rejects_invalid_magic_ranges_but_accepts_absent_fixture_blocks() {
        assert!(read_magic(&[0; 0xa0]).is_err());
        let mut bytes = vec![0; 0xa0 + MAGIC_SIZE - 1];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[0x88..0x8c].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes[0x8c..0x90].copy_from_slice(&1u32.to_le_bytes());
        assert!(read_magic(&bytes).is_err());
        bytes[0x88..0x8c].copy_from_slice(&u32::MAX.to_le_bytes());
        bytes[0x8c..0x90].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_magic(&bytes).is_err());
        bytes[0x88..0x8c].fill(0);
        assert!(read_magic(&bytes).is_err());
        bytes[0x8c..0x90].fill(0);
        assert!(read_magic(&bytes).unwrap().is_empty());
    }

    #[test]
    fn reads_item_stat_modes_signed_amounts_and_element_flags() {
        let mut bytes = vec![0; 0xa0 + ITEM_SIZE * 2];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[0x90..0x94].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes[0x94..0x98].copy_from_slice(&2u32.to_le_bytes());
        bytes[0xa0..0xa4].copy_from_slice(&123u32.to_le_bytes());
        bytes[0xa0 + 0x277] = 1;
        bytes[0xa0 + 0x28a..0xa0 + 0x28c].copy_from_slice(&(-7i16).to_le_bytes());
        bytes[0xa0 + 0x26d] = 1;
        bytes[0xa0 + ITEM_SIZE..0xa4 + ITEM_SIZE].copy_from_slice(&456u32.to_le_bytes());
        bytes[0xa4..0xa8].copy_from_slice(&[0xb2, 0xe2, 0xca, 0xd4]);
        bytes[0xa0 + 0x24..0xa0 + 0x29].copy_from_slice(b"model");
        bytes[0xa0 + 0x42] = b'7';
        bytes[0xa0 + 0x60..0xa0 + 0x64].copy_from_slice(b"desc");
        bytes[0xa0 + 0x260..0xa0 + 0x264].copy_from_slice(&1234i32.to_le_bytes());
        bytes[0xa0 + 0x264] = 11;
        bytes[0xa0 + 0x265] = 2;
        bytes[0xa0 + 0x268] = 1;
        bytes[0xa0 + 0x274] = 15;
        bytes[0xa0 + 0x275] = 2;
        bytes[0xa0 + 0x276] = 3;
        bytes[0xa0 + 0x2c0..0xa0 + 0x2c2].copy_from_slice(&2i16.to_le_bytes());
        bytes[0xa0 + 0x2f8..0xa0 + 0x2fa].copy_from_slice(&(-1i16).to_le_bytes());
        let items = read_items(&bytes).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 123);
        assert_eq!(items[0].name, "\u{6d4b}\u{8bd5}");
        assert_eq!(items[0].model, "model");
        assert_eq!(items[0].image, "7");
        assert_eq!(items[0].description, "desc");
        assert_eq!(items[0].price, 1234);
        assert_eq!(items[0].category, 11);
        assert_eq!(items[0].subtype, 2);
        assert_eq!(items[0].use_kind, 15);
        assert_eq!(items[0].use_scope, 2);
        assert_eq!(items[0].use_context, 3);
        assert_eq!(items[0].conditions[2], 2);
        assert_eq!(items[0].conditions[30], -1);
        assert_eq!(items[0].usable_by, [false, false, false, false, true]);
        assert_eq!(items[0].modes[0], 1);
        assert_eq!(items[0].amounts[3], -7);
        assert_eq!(items[0].elements, [false, false, true, false, false]);
        assert_eq!(items[1].id, 456);
        assert_eq!(items[1].amounts, [0; 12]);
    }

    #[test]
    fn rejects_truncated_or_out_of_bounds_item_blocks() {
        assert!(read_items(&[0; 0xa0]).is_err());
        let mut bytes = vec![0; 0xa0 + ITEM_SIZE - 1];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[0x90..0x94].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes[0x94..0x98].copy_from_slice(&1u32.to_le_bytes());
        assert!(read_items(&bytes).is_err());
        bytes[0x90..0x94].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_items(&bytes).is_err());
        bytes[0x90..0x94].copy_from_slice(&0u32.to_le_bytes());
        bytes[0x94..0x98].copy_from_slice(&0u32.to_le_bytes());
        assert!(read_items(&bytes).is_err());
    }
}
