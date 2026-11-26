use std::path::Path;

use crosscom::ComRc;
use radiance_scripting::{comdef::services::ISprite, services::sprite::SpriteUploader};

use super::{
    comdef::{IPal4SaveSlots, IPal4SaveSlotsImpl},
    states::persistent_state::Pal4PersistentState,
};

const WIDTH: u32 = 98;
const HEIGHT: u32 = 75;

#[derive(Default)]
struct Slot {
    status: i32,
    summary: String,
    details: [String; 4],
    leader: i32,
    level: i32,
    thumbnail: Option<ComRc<ISprite>>,
}

pub struct Pal4SaveSlots {
    slots: Vec<Slot>,
}

ComObject_Pal4SaveSlots!(super::Pal4SaveSlots);

impl Pal4SaveSlots {
    pub fn empty() -> ComRc<IPal4SaveSlots> {
        ComRc::from_object(Self { slots: Vec::new() })
    }

    pub fn snapshot(
        app_name: &str,
        catalog: Option<&super::gameplay::GameplayCatalog>,
        uploader: Option<&SpriteUploader>,
    ) -> ComRc<IPal4SaveSlots> {
        Self::snapshot_in(
            &crate::ydirs::save_dir().join(app_name).join("Save"),
            catalog,
            uploader,
        )
    }

    fn snapshot_in(
        directory: &Path,
        catalog: Option<&super::gameplay::GameplayCatalog>,
        uploader: Option<&SpriteUploader>,
    ) -> ComRc<IPal4SaveSlots> {
        let slots = (1..=Pal4PersistentState::SLOT_COUNT)
            .map(|slot| {
                let path = directory.join(format!("{slot}.json"));
                let state = std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Pal4PersistentState>(&bytes).ok());
                match state {
                    Some(state) if !state.scene_name().is_empty() => {
                        let leader = state.leader();
                        let level = state.player(leader).map_or(0, |p| p.level);
                        let seconds = state.play_seconds() as i32;
                        let location = format!("{} / {}", state.scene_name(), state.block_name());
                        let scene = catalog
                            .and_then(|catalog| {
                                catalog.scene_names.get(&(
                                    state.scene_name().to_owned(),
                                    state.block_name().to_owned(),
                                ))
                            })
                            .cloned()
                            .unwrap_or_else(|| location.clone());
                        let story = state.script_globals().first().and_then(|&plot| {
                            catalog?
                                .story_descriptions
                                .range(..=plot as i32)
                                .next_back()
                                .map(|(_, description)| description.clone())
                        });
                        let (chapter, description) = story.unwrap_or_else(|| {
                            (
                                String::new(),
                                format!("剧情进度 {}%", state.quest_percentage()),
                            )
                        });
                        let time = format!(
                            "{:02}:{:02}:{:02}",
                            seconds / 3600,
                            seconds / 60 % 60,
                            seconds % 60
                        );
                        Slot {
                            status: 1,
                            summary: format!("{slot:02}   {location}   Lv.{level}   {time}"),
                            details: [scene, time, chapter, description],
                            leader: leader as i32,
                            level,
                            thumbnail: uploader.and_then(|uploader| {
                                let image = decode_thumbnail(state.thumbnail()?)?;
                                uploader.upload(image.into_raw(), WIDTH, HEIGHT)
                            }),
                        }
                    }
                    _ => Slot {
                        status: if path.exists() { 2 } else { 0 },
                        summary: if path.exists() {
                            format!("{slot:02}   存档损坏或无法读取")
                        } else {
                            format!("{slot:02}   空白存档")
                        },
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

impl IPal4SaveSlotsImpl for Pal4SaveSlots {
    fn status(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.status)
    }
    fn summary(&self, slot: i32) -> &str {
        self.slot(slot).map_or("", |s| s.summary.as_str())
    }
    fn detail(&self, slot: i32, field: i32) -> &str {
        self.slot(slot)
            .and_then(|s| s.details.get(field as usize))
            .map_or("", |s| s.as_str())
    }
    fn leader(&self, slot: i32) -> i32 {
        self.slot(slot)
            .filter(|s| s.status == 1)
            .map_or(-1, |s| s.leader)
    }
    fn level(&self, slot: i32) -> i32 {
        self.slot(slot).map_or(0, |s| s.level)
    }
    fn thumbnail(&self, slot: i32) -> Option<ComRc<ISprite>> {
        self.slot(slot).and_then(|s| s.thumbnail.clone())
    }
}

pub(crate) fn encode_thumbnail(frame: radiance::rendering::CapturedFrame) -> Option<Vec<u8>> {
    if frame.width == 0 || frame.height == 0 {
        return None;
    }
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)?;
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
        WIDTH,
        HEIGHT,
        image::imageops::FilterType::Triangle,
    );
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(thumbnail)
        .write_to(&mut png, image::ImageOutputFormat::Png)
        .ok()?;
    Some(png)
}

fn decode_thumbnail(png: &[u8]) -> Option<image::RgbaImage> {
    if png.len() > 128 * 1024 {
        return None;
    }
    let reader = image::io::Reader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    if reader.into_dimensions().ok()? != (WIDTH, HEIGHT) {
        return None;
    }
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()
        .map(|image| image.to_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_thumbnails_crop_letterboxing_and_reject_corrupt_images() {
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
        assert_eq!(image.dimensions(), (98, 75));
        assert!(image.pixels().all(|p| p.0 == [40, 90, 160, 255]));
        assert!(decode_thumbnail(b"broken").is_none());
        assert!(decode_thumbnail(&vec![0; 128 * 1024 + 1]).is_none());
        assert!(decode_thumbnail(&png[..png.len() / 2]).is_none());
        let mut wrong_size = Vec::new();
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut wrong_size, image::ImageOutputFormat::Png)
            .unwrap();
        assert!(decode_thumbnail(&wrong_size).is_none());
    }

    #[test]
    fn save_details_and_previews_refresh_together_without_poisoning_progress() {
        let directory = std::env::temp_dir().join(format!(
            "pal4-save-preview-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("100.json");
        let mut state = Pal4PersistentState::new("test".into());
        state.set_scene("Q01".into(), "1".into());
        state.player_mut(0).level = 12;
        state.advance_play_time(3661.);
        state.add_quest_percentage(42);
        let png = encode_thumbnail(radiance::rendering::CapturedFrame {
            width: 4,
            height: 3,
            rgba: vec![200; 48],
        })
        .unwrap();
        state.set_thumbnail(Some(png.clone()));
        state.save_to_path(&path).unwrap();
        std::fs::write(directory.join("2.json"), b"broken").unwrap();
        let mut catalog = super::super::gameplay::GameplayCatalog::default();
        catalog
            .scene_names
            .insert(("Q01".into(), "1".into()), "Scene name".into());
        catalog
            .story_descriptions
            .insert(10500, ("Chapter".into(), "Story description".into()));
        catalog
            .story_descriptions
            .insert(11100, ("Future chapter".into(), "Future story".into()));
        state.set_script_globals(vec![10600]);
        state.save_to_path(&path).unwrap();
        let snapshot = Pal4SaveSlots::snapshot_in(&directory, Some(&catalog), None);
        assert_eq!(
            (snapshot.status(1), snapshot.status(2), snapshot.status(100)),
            (0, 2, 1)
        );
        assert_eq!(snapshot.detail(100, 0), "Scene name");
        assert_eq!(snapshot.detail(100, 1), "01:01:01");
        assert_eq!(snapshot.detail(100, 2), "Chapter");
        assert_eq!(snapshot.detail(100, 3), "Story description");
        assert_eq!(snapshot.leader(100), 0);
        assert_eq!(snapshot.level(100), 12);
        assert_eq!(snapshot.detail(2, 0), "");
        assert!(snapshot.thumbnail(2).is_none());
        assert_eq!(snapshot.leader(-1), -1);
        let loaded: Pal4PersistentState =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.thumbnail(), Some(png.as_slice()));
        state.set_thumbnail(Some(b"broken PNG".to_vec()));
        state.player_mut(0).level = 13;
        state.save_to_path(&path).unwrap();
        let refreshed = Pal4SaveSlots::snapshot_in(&directory, None, None);
        assert_eq!(
            refreshed.status(100),
            1,
            "bad image must not make a save unloadable"
        );
        assert_eq!(refreshed.level(100), 13);
        assert_eq!(refreshed.detail(100, 0), "Q01 / 1");
        assert_eq!(refreshed.detail(100, 3), "剧情进度 42%");
        assert_eq!(
            snapshot.level(100),
            12,
            "snapshot retains its own details and image lifetime"
        );
        state.set_thumbnail(None);
        state.save_to_path(&path).unwrap();
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("thumbnail_png")
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(directory.join("2.json")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
