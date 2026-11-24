use std::io::{self, ErrorKind};

const MAGIC: &[u8] = b"__PAL(Softstar_Ltd: Cool_J)_GameDataBase_OutFile__";
const ITEM_SIZE: usize = 800;

#[derive(Debug, Clone)]
pub struct ItemStats {
    pub id: u32,
    pub modes: [u8; 12],
    pub amounts: [i16; 12],
    pub elements: [bool; 5],
}

/// Read the stat-related fields of the compiled database's item block.
pub fn read_item_stats(bytes: &[u8]) -> io::Result<Vec<ItemStats>> {
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
    Ok(records
        .chunks_exact(ITEM_SIZE)
        .map(|record| ItemStats {
            id: u32::from_le_bytes(record[..4].try_into().unwrap()),
            modes: record[0x277..0x283].try_into().unwrap(),
            amounts: std::array::from_fn(|i| {
                i16::from_le_bytes(record[0x284 + i * 2..0x286 + i * 2].try_into().unwrap())
            }),
            elements: std::array::from_fn(|i| record[0x26b + i] != 0),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let items = read_item_stats(&bytes).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 123);
        assert_eq!(items[0].modes[0], 1);
        assert_eq!(items[0].amounts[3], -7);
        assert_eq!(items[0].elements, [false, false, true, false, false]);
        assert_eq!(items[1].id, 456);
        assert_eq!(items[1].amounts, [0; 12]);
    }

    #[test]
    fn rejects_truncated_or_out_of_bounds_item_blocks() {
        assert!(read_item_stats(&[0; 0xa0]).is_err());
        let mut bytes = vec![0; 0xa0 + ITEM_SIZE - 1];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[0x90..0x94].copy_from_slice(&0xa0u32.to_le_bytes());
        bytes[0x94..0x98].copy_from_slice(&1u32.to_le_bytes());
        assert!(read_item_stats(&bytes).is_err());
        bytes[0x90..0x94].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_item_stats(&bytes).is_err());
        bytes[0x90..0x94].copy_from_slice(&0u32.to_le_bytes());
        bytes[0x94..0x98].copy_from_slice(&0u32.to_le_bytes());
        assert!(read_item_stats(&bytes).is_err());
    }
}
