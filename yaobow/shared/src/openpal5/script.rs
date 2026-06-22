//! PAL5 game-script index + source loader (game-agnostic data side).
//!
//! PAL5's scripts live in `script.pkg` (mounted at `/script`) as
//! SDFA-encrypted Lua 5.1 source; `packfs` transparently decrypts them on
//! read, so [`ScriptIndex::load_source`] returns plain Lua source bytes.
//!
//! `Config/data/scriptlist.ini` (GBK, mounted at
//! `/Config/data/scriptlist.ini`) maps a numeric script id to a script
//! name, grouped by `[section]`. The on-disk path is
//! `/script/<name>.lua` for the root section `[.]` and
//! `/script/<section>/<name>.lua` otherwise. The script's *entry
//! function* shares the file stem (e.g. id `7001` →
//! `mainline/m001_1.lua` → `m001_1()`), which the Lua `CallScript`
//! dispatch calls after loading.

use std::collections::HashMap;

use encoding::{DecoderTrap, Encoding};
use mini_fs::MiniFs;

use common::store_ext::StoreExt2;

const SCRIPTLIST_PATH: &str = "/Config/data/scriptlist.ini";

#[derive(Clone, Debug)]
pub struct ScriptEntry {
    /// Entry function name (the file stem, e.g. `m001_1`).
    pub name: String,
    /// VFS path of the script source, e.g. `/script/mainline/m001_1.lua`.
    pub vfs_path: String,
}

pub struct ScriptIndex {
    by_id: HashMap<u32, ScriptEntry>,
}

impl ScriptIndex {
    /// Parse `scriptlist.ini` from the mounted vfs.
    pub fn load(vfs: &MiniFs) -> anyhow::Result<Self> {
        let raw = vfs.read_to_end(SCRIPTLIST_PATH)?;
        let text = encoding::all::GBK
            .decode(&raw, DecoderTrap::Replace)
            .unwrap_or_else(|_| String::from_utf8_lossy(&raw).into_owned());
        Ok(Self::parse(&text))
    }

    fn parse(text: &str) -> Self {
        let mut by_id = HashMap::new();
        let mut section = String::from(".");

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }
            let Some((id_str, name)) = line.split_once('=') else {
                continue;
            };
            let id_str = id_str.trim();
            let name = name.trim();
            // Strip an inline comment after the value, if any.
            let name = name.split(';').next().unwrap_or(name).trim();
            let Ok(id) = id_str.parse::<u32>() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }

            let vfs_path = if section == "." {
                format!("/script/{}.lua", name)
            } else {
                format!("/script/{}/{}.lua", section, name)
            };
            by_id.insert(
                id,
                ScriptEntry {
                    name: name.to_string(),
                    vfs_path,
                },
            );
        }

        Self { by_id }
    }

    pub fn entry(&self, id: u32) -> Option<&ScriptEntry> {
        self.by_id.get(&id)
    }

    /// Load the (already SDFA-decrypted) Lua source for `id`.
    pub fn load_source(&self, vfs: &MiniFs, id: u32) -> anyhow::Result<(String, Vec<u8>)> {
        let entry = self
            .entry(id)
            .ok_or_else(|| anyhow::anyhow!("script id {} not in scriptlist.ini", id))?;
        let src = vfs.read_to_end(&entry.vfs_path)?;
        Ok((entry.name.clone(), src))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sections_and_paths() {
        let ini = "[.]\n1=NewGame\n2=prelogue\n\n[mainline]\n7001=m001_1\n9601=macro\n";
        let idx = ScriptIndex::parse(ini);
        assert_eq!(idx.entry(1).unwrap().vfs_path, "/script/NewGame.lua");
        assert_eq!(idx.entry(1).unwrap().name, "NewGame");
        assert_eq!(
            idx.entry(7001).unwrap().vfs_path,
            "/script/mainline/m001_1.lua"
        );
        assert_eq!(idx.entry(9601).unwrap().name, "macro");
    }
}
