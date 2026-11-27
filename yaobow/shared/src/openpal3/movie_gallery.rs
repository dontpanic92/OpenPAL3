use std::io;
use std::path::{Path, PathBuf};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use mini_fs::MiniFs;
use serde::{Deserialize, Serialize};

use super::comdef::{IPal3MovieGallery, IPal3MovieGalleryImpl};
use crate::ydirs;

// Original PAL3 movie catalog (0x5955dc / 0x5b0efc); PAL3A is separate.
pub const MOVIES: [&str; 52] = [
    "Pal3op", "1A", "1C", "1K", "1N", "2E", "2M1", "2M2", "3F", "3H", "3L", "4G", "4O1", "4O2",
    "5F", "6G1", "6G2", "6G3", "7C", "7L", "9E", "9I", "10A", "10F", "11G", "11J", "12E", "12F",
    "13D", "13I1", "13I2", "13I3", "13J", "14D", "14I1", "14I2", "14K", "14L", "15B1", "15B2",
    "15C1", "15C2", "15F", "15G", "15H1", "15H2", "END1", "END2", "END3", "END4", "END5", "END",
];

#[derive(Default, Deserialize, Serialize)]
pub struct MovieUnlocks {
    // Same bit ordering as the original MOVIE_A / MOVIE_B registry DWORDs.
    masks: [u32; 2],
}

impl MovieUnlocks {
    pub fn path() -> PathBuf {
        ydirs::save_dir()
            .join(crate::GameType::PAL3.app_name())
            .join("movies.json")
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read(path) {
            Ok(data) => Ok(serde_json::from_slice(&data)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn unlocked(&self, index: i32) -> bool {
        usize::try_from(index)
            .ok()
            .filter(|&i| i < MOVIES.len())
            .is_some_and(|i| self.masks[i / 32] & (1 << (i % 32)) != 0)
    }

    fn record(path: &Path, name: &str) -> anyhow::Result<()> {
        let Some(index) = MOVIES
            .iter()
            .position(|movie| movie.eq_ignore_ascii_case(name))
        else {
            return Ok(());
        };
        let mut progress = Self::load(path)?;
        if progress.unlocked(index as i32) {
            return Ok(());
        }
        progress.masks[index / 32] |= 1 << (index % 32);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(&progress)?)?;
        if let Err(error) = std::fs::rename(&temporary, path) {
            let _ = std::fs::remove_file(temporary);
            return Err(error.into());
        }
        Ok(())
    }
}

pub fn record_movie(name: &str) {
    if let Err(error) = MovieUnlocks::record(&MovieUnlocks::path(), name) {
        log::warn!("Cannot save PAL3 movie collection: {error}");
    }
}

pub struct Pal3MovieGallery {
    unlocks: MovieUnlocks,
    titles: Vec<String>,
}

ComObject_Pal3MovieGallery!(super::Pal3MovieGallery);

impl Pal3MovieGallery {
    pub fn snapshot(vfs: &MiniFs) -> ComRc<IPal3MovieGallery> {
        let unlocks = MovieUnlocks::load(&MovieUnlocks::path()).unwrap_or_else(|error| {
            log::warn!("Cannot read PAL3 movie collection: {error}");
            MovieUnlocks::default()
        });
        let titles = vfs
            .read_to_end_from_gbk("/basedata/basedata/AssisLang.lang")
            .map(|text| movie_titles(&text))
            .unwrap_or_else(|error| {
                log::warn!("Cannot read PAL3 movie titles: {error}");
                MOVIES.iter().map(|name| name.to_string()).collect()
            });
        ComRc::from_object(Self { unlocks, titles })
    }
}

impl IPal3MovieGalleryImpl for Pal3MovieGallery {
    fn unlocked(&self, index: i32) -> bool {
        self.unlocks.unlocked(index)
    }

    fn title(&self, index: i32) -> &str {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.titles.get(i))
            .map(String::as_str)
            .unwrap_or("")
    }
}

fn movie_titles(text: &str) -> Vec<String> {
    let mut titles: Vec<_> = MOVIES.iter().map(|name| name.to_string()).collect();
    for line in text.lines() {
        let Some((key, value)) = line.trim_start().split_once('$') else {
            continue;
        };
        let Some(index) = key
            .trim()
            .strip_prefix("Lang")
            .and_then(|id| id.parse::<usize>().ok())
            .and_then(|id| id.checked_sub(290))
            .filter(|&i| i < MOVIES.len())
        else {
            continue;
        };
        let Some((value, _)) = value.split_once('&') else {
            continue;
        };
        titles[index] = value.replace("\\n", "\n");
    }
    titles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_preserves_both_native_masks_across_updates_and_reloads() {
        let dir = std::env::temp_dir().join(format!(
            "pal3-movies-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("movies.json");
        assert!(!MovieUnlocks::load(&path).unwrap().unlocked(0));
        MovieUnlocks::record(&path, "LOGO").unwrap();
        assert!(!path.exists());
        for movie in ["pal3OP", "13i3", "13J", "end"] {
            MovieUnlocks::record(&path, movie).unwrap();
        }
        let progress = MovieUnlocks::load(&path).unwrap();
        assert_eq!(progress.masks, [0x80000001, 0x80001]);
        for index in -1..=52 {
            assert_eq!(progress.unlocked(index), [0, 31, 32, 51].contains(&index));
        }
        let before = std::fs::read(&path).unwrap();
        MovieUnlocks::record(&path, "Pal3op").unwrap();
        assert_eq!(before, std::fs::read(&path).unwrap());
        std::fs::write(&path, b"broken progress").unwrap();
        assert!(MovieUnlocks::record(&path, "1A").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken progress");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn titles_use_original_numbered_keys_not_comments_or_line_order() {
        let titles = movie_titles(
            "; Lang290 $comment&\nLang291 $one&\nLang290 $opening&\n\
             Lang340 $first&\nLang340 $last& ; original data has a duplicate\n\
             Lang341 $end\\ncredits&\nLang342 $outside&\nLang292 $unterminated",
        );
        assert_eq!(&titles[..3], ["opening", "one", "1C"]);
        assert_eq!(titles[50], "last");
        assert_eq!(titles[51], "end\ncredits");
    }
}
