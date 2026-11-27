use std::path::Path;

use crosscom::ComRc;
use fileformats::pal3::scene_names::SceneNames;
use radiance_scripting::{comdef::services::ISprite, services::sprite::SpriteUploader};

use super::{
    comdef::{IPal3SaveSlots, IPal3SaveSlotsImpl},
    states::persistent_state::PersistentState,
};

pub const SAVE_SLOT_COUNT: i32 = 36;

#[derive(Default)]
struct Slot {
    status: i32,
    scene: String,
    seconds: i32,
    money: i32,
    members: Vec<i32>,
    levels: [i32; 5],
    thumbnail: Option<ComRc<ISprite>>,
}

pub struct Pal3SaveSlots {
    slots: Vec<Slot>,
}

ComObject_Pal3SaveSlots!(super::Pal3SaveSlots);

impl Pal3SaveSlots {
    pub fn snapshot(app_name: &str) -> ComRc<IPal3SaveSlots> {
        Self::snapshot_with_thumbnails(app_name, None, None)
    }

    pub fn snapshot_with_thumbnails(
        app_name: &str,
        uploader: Option<&SpriteUploader>,
        scene_names: Option<&SceneNames>,
    ) -> ComRc<IPal3SaveSlots> {
        Self::snapshot_in_with_thumbnails(
            app_name,
            &crate::ydirs::save_dir().join(app_name).join("Save"),
            uploader,
            scene_names,
        )
    }

    #[cfg(test)]
    fn snapshot_in(app_name: &str, directory: &Path) -> ComRc<IPal3SaveSlots> {
        Self::snapshot_in_with_thumbnails(app_name, directory, None, None)
    }

    fn snapshot_in_with_thumbnails(
        app_name: &str,
        directory: &Path,
        uploader: Option<&SpriteUploader>,
        scene_names: Option<&SceneNames>,
    ) -> ComRc<IPal3SaveSlots> {
        let slots = (1..=SAVE_SLOT_COUNT)
            .map(|index| {
                let path = directory.join(format!("{index}.json"));
                match PersistentState::load_from_path(app_name, &path) {
                    Ok(state) => Slot {
                        status: 1,
                        scene: {
                            let scene = state.scene_name().unwrap();
                            let block = state.sub_scene_name().unwrap();
                            scene_names
                                .and_then(|names| names.lookup(&scene, &block))
                                .map(str::to_string)
                                .unwrap_or_else(|| format!("{scene} / {block}"))
                        },
                        seconds: state.play_seconds(),
                        money: state.money(),
                        members: state.party().members().collect(),
                        levels: std::array::from_fn(|role| {
                            state
                                .stats()
                                .character(role as i32)
                                .map_or(0, |c| c.level())
                        }),
                        thumbnail: uploader.and_then(|uploader| {
                            let rgba = decode_thumbnail(state.thumbnail()?)?;
                            uploader.upload(rgba.into_raw(), 160, 120)
                        }),
                    },
                    Err(_) => Slot {
                        status: if path.exists() { 2 } else { 0 },
                        ..Slot::default()
                    },
                }
            })
            .collect();
        ComRc::from_object(Self { slots })
    }

    fn slot(&self, slot: i32) -> Option<&Slot> {
        slot.checked_sub(1).and_then(|i| self.slots.get(i as usize))
    }
}

impl IPal3SaveSlotsImpl for Pal3SaveSlots {
    fn thumbnail(&self, slot: i32) -> Option<ComRc<ISprite>> {
        self.slot(slot).and_then(|s| s.thumbnail.clone())
    }
    fn slot_count(&self) -> i32 {
        SAVE_SLOT_COUNT
    }
    fn status(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.status)
    }
    fn scene(&self, slot: i32) -> &str {
        self.slot(slot).map_or("", |s| &s.scene)
    }
    fn play_seconds(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.seconds)
    }
    fn money(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.money)
    }
    fn member_role(&self, slot: i32, index: i32) -> i32 {
        self.slot(slot)
            .and_then(|s| s.members.get(index as usize))
            .copied()
            .unwrap_or(-1)
    }

    fn role_level(&self, slot: i32, role: i32) -> i32 {
        self.slot(slot)
            .and_then(|s| s.levels.get(role as usize))
            .copied()
            .unwrap_or(0)
    }
}

pub(crate) fn encode_thumbnail(frame: radiance::rendering::CapturedFrame) -> Option<Vec<u8>> {
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)?;
    if frame.width == 0 || frame.height == 0 {
        return None;
    }
    let scale = (frame.width as f64 / 4.).min(frame.height as f64 / 3.);
    let width = (scale * 4.) as u32;
    let height = (scale * 3.) as u32;
    if width == 0 || height == 0 {
        return None;
    }
    let crop = image::imageops::crop_imm(
        &image,
        (frame.width - width) / 2,
        (frame.height - height) / 2,
        width,
        height,
    );
    let thumbnail = image::imageops::resize(
        &crop.to_image(),
        160,
        120,
        image::imageops::FilterType::Triangle,
    );
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(thumbnail)
        .write_to(&mut png, image::ImageOutputFormat::Png)
        .ok()?;
    Some(png)
}

fn decode_thumbnail(png: &[u8]) -> Option<image::RgbaImage> {
    if png.len() > 256 * 1024 {
        return None;
    }
    let reader = image::io::Reader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    if reader.into_dimensions().ok()? != (160, 120) {
        return None;
    }
    Some(
        image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .ok()?
            .to_rgba8(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_crop_letterboxing_and_reject_invalid_or_oversized_images() {
        let mut frame = image::RgbaImage::from_pixel(800, 900, image::Rgba([0, 0, 0, 255]));
        for y in 150..750 {
            for x in 0..800 {
                frame.put_pixel(x, y, image::Rgba([40, 90, 160, 255]));
            }
        }
        let png = encode_thumbnail(radiance::rendering::CapturedFrame {
            width: 800,
            height: 900,
            rgba: frame.into_raw(),
        })
        .unwrap();
        let image = decode_thumbnail(&png).unwrap();
        assert_eq!(image.dimensions(), (160, 120));
        assert!(image.pixels().all(|p| p.0 == [40, 90, 160, 255]));
        assert!(decode_thumbnail(b"broken PNG").is_none());
        assert!(decode_thumbnail(&vec![0; 256 * 1024 + 1]).is_none());
        assert!(
            encode_thumbnail(radiance::rendering::CapturedFrame {
                width: 2,
                height: 2,
                rgba: vec![0; 3],
            })
            .is_none()
        );
        let mut wrong_size = Vec::new();
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut wrong_size, image::ImageOutputFormat::Png)
            .unwrap();
        assert!(decode_thumbnail(&wrong_size).is_none());
        let mut truncated = png;
        truncated.truncate(truncated.len() / 2);
        assert!(decode_thumbnail(&truncated).is_none());
    }

    #[test]
    fn save_snapshots_distinguish_empty_corrupt_and_complete_slots_and_refresh_after_overwrite() {
        let directory = std::env::temp_dir().join(format!(
            "pal3-save-browser-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("36.json");
        let mut state = PersistentState::new("ignored-save-app".into());
        state.set_scene_name("YZZ".into(), "AN".into());
        state.party_mut().set_in_team(1, true);
        state.add_money(321);
        state.advance_play_time(3661.0);
        let thumbnail = encode_thumbnail(radiance::rendering::CapturedFrame {
            width: 4,
            height: 3,
            rgba: vec![200; 4 * 3 * 4],
        })
        .unwrap();
        state.set_thumbnail(Some(thumbnail.clone()));
        state.save_to_path(&path).unwrap();
        std::fs::write(directory.join("1.json"), b"{broken").unwrap();
        std::fs::write(
            directory.join("2.json"),
            serde_json::to_string(&PersistentState::new("test".into())).unwrap(),
        )
        .unwrap();

        let snapshot = Pal3SaveSlots::snapshot_in("test", &directory);
        assert_eq!(snapshot.status(1), 2);
        assert_eq!(snapshot.status(2), 2);
        assert_eq!(snapshot.status(3), 0);
        assert_eq!(snapshot.status(36), 1);
        assert_eq!(snapshot.scene(36), "YZZ / AN");
        let names = SceneNames::parse("BEGIN,YZZ\ndefault $Town&\nan $Market&\nEND");
        let localized =
            Pal3SaveSlots::snapshot_in_with_thumbnails("test", &directory, None, Some(&names));
        assert_eq!(localized.scene(36), "Market");
        assert_eq!(localized.status(1), 2);
        assert_eq!(localized.scene(1), "");
        assert_eq!(
            PersistentState::load_from_path("test", &path)
                .unwrap()
                .scene_name()
                .as_deref(),
            Some("YZZ"),
            "display names must not replace saved scene identifiers"
        );
        assert_eq!(snapshot.play_seconds(36), 3661);
        assert_eq!(snapshot.member_role(36, 1), 1);
        assert_eq!(snapshot.member_role(36, -1), -1);
        assert_eq!(
            snapshot.role_level(36, 0),
            state.stats().character(0).unwrap().level()
        );
        assert_eq!(snapshot.money(36), 321);
        assert_eq!(
            PersistentState::load_from_path("test", &path)
                .unwrap()
                .thumbnail(),
            Some(thumbnail.as_slice())
        );
        assert!(
            PersistentState::load_from_path("test", &path)
                .unwrap()
                .thumbnail()
                .and_then(decode_thumbnail)
                .is_some()
        );

        state.add_money(10);
        state.set_thumbnail(None);
        state.save_to_path(&path).unwrap();
        assert_eq!(snapshot.money(36), 321);
        assert_eq!(
            Pal3SaveSlots::snapshot_in("test", &directory).money(36),
            331
        );
        let loaded = PersistentState::load_from_path("test", &path).unwrap();
        assert_eq!(loaded.app_name(), "test");
        assert_eq!(loaded.money(), 331);
        assert!(
            loaded.thumbnail().is_none(),
            "overwrites must remove stale images"
        );
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("thumbnail_png"),
            "legacy saves without the optional field must still load"
        );
        assert_eq!(loaded.party().members().collect::<Vec<_>>(), vec![0, 1]);
        assert!(PersistentState::load_from_path("test", &directory.join("1.json")).is_err());

        let blocked = directory.join("blocked.json");
        std::fs::create_dir(&blocked).unwrap();
        assert!(state.save_to_path(&blocked).is_err());
        assert!(blocked.is_dir());
        assert!(
            !blocked
                .with_extension(format!("json.{}.tmp", std::process::id()))
                .exists()
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
