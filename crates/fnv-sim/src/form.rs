//! Global (load-order) FormIDs.
//!
//! Every FormID inside `fnv-sim` is *global*: bits 24 and up are the load-order slot of
//! the plugin that owns the form, as produced by [`fnv_esm::LoadOrder::to_global`], and
//! the low 24 bits the record number inside that plugin. File-local FormIDs never leave
//! the loaders (`forms`, `refs`).
//!
//! The runtime ID is 64 bits wide (upstream #1758): `(slot << 24) | record`, the same
//! formula as the 32-bit IDs of the original with a 16-bit slot. For slots 0x00-0xFE the
//! number is the original's 32-bit ID unchanged; slot 0xFF holds forms created at run
//! time; plugin slots past 0xFE skip to 0x100 [design: NVForever's own extension, the
//! original loads at most 255 plugins, documented: UESP "Tes4Mod:Mod File Format"].

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FormId(pub u64);

impl FormId {
    /// The player's placed reference. [hypothesis: 4,566 FormID fields in the masters
    /// point at 0x14, docs/design/layer1.md §2.4; checked by `fnvtool sim forms`]
    pub const PLAYER_REF: FormId = FormId(0x14);
    /// The player's base NPC_ record. [hypothesis; checked by `fnvtool sim forms`]
    pub const PLAYER_BASE: FormId = FormId(0x7);
    /// First FormID handed out to references created at run time (`PlaceAtMe`, dropped
    /// items). The 0xFF load-order slot is never used by a plugin (at most 255 load,
    /// indices 0x00–0xFE). [hypothesis: the original engine uses the same slot for
    /// created forms; to be confirmed by a savegame or xNVSE capture]
    pub const FIRST_DYNAMIC: FormId = FormId(0xFF00_0000);

    /// Slot of the runtime-created forms (`FIRST_DYNAMIC`).
    pub const DYNAMIC_SLOT: u16 = 0xFF;

    /// The load-order slot (bits 24-39) [design: upstream #1758].
    pub fn slot(self) -> u16 {
        slot_of(self.0)
    }

    /// The record number inside its plugin (the low 24 bits).
    pub fn record(self) -> u32 {
        (self.0 & 0x00FF_FFFF) as u32
    }

    /// The low byte of the slot, the "mod index" a 32-bit ID carries. Only meaningful
    /// for slots 0x00-0xFF; prefer [`FormId::slot`].
    pub fn plugin_index(self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// A form created at run time (slot 0xFF).
    pub fn is_dynamic(self) -> bool {
        self.slot() == Self::DYNAMIC_SLOT
    }

    /// The ID as the original's 32 bits, when it fits (slots 0x00-0xFF).
    pub fn to_u32(self) -> Option<u32> {
        u32::try_from(self.0).ok()
    }
}

/// The load-order slot of a raw global FormID (bits 24-39).
pub const fn slot_of(id: u64) -> u16 {
    ((id >> 24) & 0xFFFF) as u16
}

/// A raw global FormID from a slot and a record number.
pub const fn make_id(slot: u16, record: u32) -> u64 {
    ((slot as u64) << 24) | (record & 0x00FF_FFFF) as u64
}

impl From<u32> for FormId {
    fn from(v: u32) -> FormId {
        FormId(u64::from(v))
    }
}

impl fmt::Display for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

impl fmt::Debug for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a hex FormID: {0:?}")]
pub struct BadFormId(pub String);

impl FromStr for FormId {
    type Err = BadFormId;
    fn from_str(s: &str) -> Result<FormId, BadFormId> {
        let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
        // Up to 10 hex digits: a 16-bit slot and a 24-bit record (bits 40-63 unused).
        if hex.is_empty() || hex.len() > 10 {
            return Err(BadFormId(s.to_string()));
        }
        u64::from_str_radix(hex, 16).map(FormId).map_err(|_| BadFormId(s.to_string()))
    }
}

impl Serialize for FormId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FormId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<FormId, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_forms() {
        assert_eq!("0x0001F4A2".parse::<FormId>().unwrap(), FormId(0x1F4A2));
        assert_eq!("1f4a2".parse::<FormId>().unwrap(), FormId(0x1F4A2));
        assert!("".parse::<FormId>().is_err());
        assert!("0x12345678901".parse::<FormId>().is_err());
        assert_eq!("0x0100000801".parse::<FormId>().unwrap(), FormId(0x01_0000_0801));
        assert_eq!(FormId(0x01_0000_0801).to_string(), "0x100000801");
        assert_eq!(FormId(0x01_0000_0801).slot(), 0x100);
        assert_eq!(FormId(0x01_0000_0801).record(), 0x801);
        assert!(!FormId(0x01_0000_0801).is_dynamic());
        assert!("zz".parse::<FormId>().is_err());
        assert_eq!(FormId(0x14).to_string(), "0x00000014");
        let j = serde_json::to_string(&FormId(0xFF000001)).unwrap();
        assert_eq!(j, "\"0xFF000001\"");
        assert_eq!(serde_json::from_str::<FormId>(&j).unwrap(), FormId(0xFF000001));
        assert!(FormId(0xFF000001).is_dynamic());
        assert!(!FormId(0xFE000001).is_dynamic());
    }
}
