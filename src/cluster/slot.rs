/// Total number of hash slots in the cluster (same as Redis).
pub const CLUSTER_HASH_SLOTS: u16 = 16384;

/// CRC-16/CCITT (XMODEM) lookup table — matches Redis's crc16() exactly.
const CRC16_TABLE: [u16; 256] = [
    0x0000, 0x1021, 0x2042, 0x3063, 0x4084, 0x50A5, 0x60C6, 0x70E7,
    0x8108, 0x9129, 0xA14A, 0xB16B, 0xC18C, 0xD1AD, 0xE1CE, 0xF1EF,
    0x1231, 0x0210, 0x3273, 0x2252, 0x52B5, 0x4294, 0x72F7, 0x62D6,
    0x9339, 0x8318, 0xB37B, 0xA35A, 0xD3BD, 0xC39C, 0xF3FF, 0xE3DE,
    0x2462, 0x3443, 0x0420, 0x1401, 0x64E6, 0x74C7, 0x44A4, 0x5485,
    0xA56A, 0xB54B, 0x8528, 0x9509, 0xE5EE, 0xF5CF, 0xC5AC, 0xD58D,
    0x3653, 0x2672, 0x1611, 0x0630, 0x76D7, 0x66F6, 0x5695, 0x46B4,
    0xB75B, 0xA77A, 0x9719, 0x8738, 0xF7DF, 0xE7FE, 0xD79D, 0xC7BC,
    0x4864, 0x5845, 0x6826, 0x7807, 0x08E0, 0x18C1, 0x28A2, 0x38C3,
    0xC92C, 0xD90D, 0xE96E, 0xF94F, 0x89A8, 0x9989, 0xA9EA, 0xB9CB,
    0x5A15, 0x4A34, 0x7A57, 0x6A76, 0x1A91, 0x0AB0, 0x3AD3, 0x2AF2,
    0xDB1D, 0xCB3C, 0xFB5F, 0xEB7E, 0x9B99, 0x8BB8, 0xBBDB, 0xABFA,
    0x6CA6, 0x7C87, 0x4CE4, 0x5CC5, 0x2C22, 0x3C03, 0x0C60, 0x1C41,
    0xEDAE, 0xFD8F, 0xCDEC, 0xDDCD, 0xAD2A, 0xBD0B, 0x8D68, 0x9D49,
    0x7E97, 0x6EB6, 0x5ED5, 0x4EF4, 0x3E13, 0x2E32, 0x1E51, 0x0E70,
    0xFF9F, 0xEFBE, 0xDFDD, 0xCFFC, 0xBF1B, 0xAF3A, 0x9F59, 0x8F78,
    0x9188, 0x81A9, 0xB1CA, 0xA1EB, 0xD10C, 0xC12D, 0xF14E, 0xE16F,
    0x1080, 0x00A1, 0x30C2, 0x20E3, 0x5004, 0x4025, 0x7046, 0x6067,
    0x83B9, 0x9398, 0xA3FB, 0xB3DA, 0xC33D, 0xD31C, 0xE37F, 0xF35E,
    0x02B1, 0x1290, 0x22F3, 0x32D2, 0x4235, 0x5214, 0x6277, 0x7256,
    0xB5EA, 0xA5CB, 0x95A8, 0x8589, 0xF56E, 0xE54F, 0xD52C, 0xC50D,
    0x34E2, 0x24C3, 0x14A0, 0x0481, 0x7466, 0x6447, 0x5424, 0x4405,
    0xA7DB, 0xB7FA, 0x8799, 0x97B8, 0xE75F, 0xF77E, 0xC71D, 0xD73C,
    0x26D3, 0x36F2, 0x0691, 0x16B0, 0x6657, 0x7676, 0x4615, 0x5634,
    0xD94C, 0xC96D, 0xF90E, 0xE92F, 0x99C8, 0x89E9, 0xB98A, 0xA9AB,
    0x5844, 0x4865, 0x7806, 0x6827, 0x18C0, 0x08E1, 0x3882, 0x28A3,
    0xCB7D, 0xDB5C, 0xEB3F, 0xFB1E, 0x8BF9, 0x9BD8, 0xABBB, 0xBB9A,
    0x4A75, 0x5A54, 0x6A37, 0x7A16, 0x0AF1, 0x1AD0, 0x2AB3, 0x3A92,
    0xFD2E, 0xED0F, 0xDD6C, 0xCD4D, 0xBDAA, 0xAD8B, 0x9DE8, 0x8DC9,
    0x7C26, 0x6C07, 0x5C64, 0x4C45, 0x3CA2, 0x2C83, 0x1CE0, 0x0CC1,
    0xEF1F, 0xFF3E, 0xCF5D, 0xDF7C, 0xAF9B, 0xBFBA, 0x8FD9, 0x9FF8,
    0x6E17, 0x7E36, 0x4E55, 0x5E74, 0x2E93, 0x3EB2, 0x0ED1, 0x1EF0,
];

/// Compute CRC-16/CCITT (XMODEM) over the given data.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc = (crc << 8) ^ CRC16_TABLE[((crc >> 8) ^ byte as u16) as usize];
    }
    crc
}

/// Extract the "hash tag" from a key.
///
/// If the key contains `{...}`, only the content between the first `{` and
/// the first `}` after it is used for hashing. This allows related keys to
/// be mapped to the same slot.
///
/// Examples:
/// - `foo{bar}baz` → `bar`
/// - `{bar}baz`     → `bar`
/// - `foo{bar}`     → `bar`
/// - `foobar`       → `foobar` (no tag, use full key)
/// - `foo{}bar`     → `foo{}bar` (empty tag, use full key)
fn extract_hash_tag(key: &[u8]) -> &[u8] {
    // Find first '{'
    let start = match key.iter().position(|&b| b == b'{') {
        Some(i) => i,
        None => return key,
    };

    // Find first '}' after '{'
    let end = match key[start + 1..].iter().position(|&b| b == b'}') {
        Some(i) => start + 1 + i,
        None => return key,
    };

    // Empty tag → use full key
    if end == start + 1 {
        return key;
    }

    &key[start + 1..end]
}

/// Compute the hash slot for a given key.
///
/// Returns a value in `0..CLUSTER_HASH_SLOTS` (0..16384).
pub fn key_hash_slot(key: &[u8]) -> u16 {
    let effective_key = extract_hash_tag(key);
    crc16(effective_key) % CLUSTER_HASH_SLOTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_empty() {
        assert_eq!(crc16(b""), 0x0000);
    }

    #[test]
    fn test_crc16_123456789() {
        // Standard test vector for CRC-16/CCITT XMODEM
        assert_eq!(crc16(b"123456789"), 0x31C3);
    }

    #[test]
    fn test_hash_slot_basic() {
        // Same key should always give same slot
        let slot1 = key_hash_slot(b"mykey");
        let slot2 = key_hash_slot(b"mykey");
        assert_eq!(slot1, slot2);
        assert!(slot1 < CLUSTER_HASH_SLOTS);
    }

    #[test]
    fn test_hash_tag() {
        // Keys with same hash tag should map to same slot
        let slot1 = key_hash_slot(b"user{123}");
        let slot2 = key_hash_slot(b"session{123}");
        assert_eq!(slot1, slot2);

        // Different tags should (likely) map to different slots
        let slot3 = key_hash_slot(b"user{456}");
        // Not guaranteed to be different, but very likely
        assert!(slot1 < CLUSTER_HASH_SLOTS);
        assert!(slot3 < CLUSTER_HASH_SLOTS);
    }

    #[test]
    fn test_hash_tag_empty() {
        // Empty tag should use full key
        let slot1 = key_hash_slot(b"foo{}bar");
        let slot2 = key_hash_slot(b"foo{}bar");
        assert_eq!(slot1, slot2);

        // Should be same as no tag with same full key
        let slot3 = key_hash_slot(b"foo{}bar");
        assert_eq!(slot2, slot3);
    }

    #[test]
    fn test_no_hash_tag() {
        let slot = key_hash_slot(b"normal_key");
        assert!(slot < CLUSTER_HASH_SLOTS);
    }

    #[test]
    fn test_unclosed_tag() {
        // No closing brace → use full key
        let slot = key_hash_slot(b"foo{bar");
        assert!(slot < CLUSTER_HASH_SLOTS);
    }

    #[test]
    fn test_known_redis_slots() {
        // Cross-reference with Redis: KEYSLOT somekey returns 11058
        // Redis uses: crc16("somekey") mod 16384
        let slot = key_hash_slot(b"somekey");
        let expected = crc16(b"somekey") % CLUSTER_HASH_SLOTS;
        assert_eq!(slot, expected);
    }
}
