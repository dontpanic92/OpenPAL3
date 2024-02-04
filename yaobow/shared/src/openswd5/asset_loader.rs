use std::{
    io::{BufReader, Cursor},
    path::PathBuf,
    rc::Rc,
};

use common::{read_ext::ReadExt, store_ext::StoreExt2};
use encoding::{DecoderTrap, Encoding};
use fileformats::{
    atp::{AtpEntry, AtpEntryData4, AtpFile},
    binrw::BinRead,
    c00::C00,
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    rendering::{ComponentFactory, Sprite},
    utils::SeekRead,
};

use crate::GameType;

pub struct AssetLoader {
    game: GameType,
    vfs: Rc<MiniFs>,
    component_factory: Rc<dyn ComponentFactory>,
    index: Vec<Option<AtpEntry>>,
}

impl AssetLoader {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        vfs: Rc<MiniFs>,
        game: GameType,
    ) -> Rc<Self> {
        let index = Self::load_index(&vfs).unwrap();
        Rc::new(Self {
            game,
            vfs,
            component_factory,
            index,
        })
    }

    pub fn load_main_script(&self) -> anyhow::Result<Vec<u8>> {
        let content = self.vfs.read_to_end(self.main_script_path())?;
        let mut reader = Cursor::new(content);
        let c00 = C00::read(&mut reader)?;

        let lzo: minilzo_rs::LZO = minilzo_rs::LZO::init()?;
        let out = lzo.decompress(&c00.data, c00.header.original_size as usize)?;

        Ok(out)
    }

    pub fn load_sound(&self, sound_id: i32) -> anyhow::Result<Vec<u8>> {
        let path = format!("/Sound/SoundDB/{}.mp3", sound_id);
        let content = self.vfs.read_to_end(path)?;

        Ok(content)
    }

    pub fn load_music(&self, music_id: i32) -> anyhow::Result<Vec<u8>> {
        let path = format!("/Music/Music/{}.mp3", music_id);
        let content = self.vfs.read_to_end(path)?;

        Ok(content)
    }

    pub fn load_story_pic(&self, pic_id: i32) -> anyhow::Result<Sprite> {
        let atp_entry = self.index[(pic_id + -1) as usize]
            .as_ref()
            .ok_or(anyhow::anyhow!("No such pic {pic_id}"))?;

        let data4 = atp_entry.data4.as_ref().unwrap();
        match data4 {
            AtpEntryData4::Data1(_) => {
                anyhow::bail!("Unsupported data41 type in load_story_pic: {:?}", pic_id);
            }
            AtpEntryData4::Data5(d45) => {
                let path = d45.unknown9.as_ref().unwrap().first().unwrap().path.clone();
                let path = encoding::all::BIG5_2003
                    .decode(&path.data, DecoderTrap::Ignore)
                    .map_err(|_| anyhow::anyhow!("Cannot decode big5 string"))?;
                let path = PathBuf::from("/Texture/Texture").join(
                    PathBuf::from(PathBuf::from(path).file_stem().unwrap()).with_extension("png"),
                );

                let data = self.vfs.read_to_end(path)?;
                let width = (&data[0..4]).read_u32_le().unwrap();
                let height = (&data[4..8]).read_u32_le().unwrap();
                let data = &data[8..];
                let image = image::RgbaImage::from_raw(width, height, data.to_vec())
                    .map(|img| image::DynamicImage::ImageRgba8(img))
                    .ok_or(anyhow::anyhow!("Cannot create image"))?
                    .to_rgba8();

                Ok(Sprite::load_from_image(
                    image,
                    self.component_factory.as_ref(),
                ))
            }
            _ => anyhow::bail!("Unsupported data4 type"),
        }
    }

    pub fn load_movie_data(&self, movie_id: u32) -> anyhow::Result<Box<dyn SeekRead>> {
        let path = format!("/movie/movie{:0>2}.bik", movie_id);

        println!("Loading movie: {}", path);
        let content = self.vfs.open(path)?;

        Ok(Box::new(BufReader::new(content)))
    }

    fn main_script_path(&self) -> String {
        match self.game {
            GameType::SWD5 => "/Script/0000.C01".to_string(),
            GameType::SWDHC => "/Text/main/0000.C01".to_string(),
            GameType::SWDCF => "/Text/Off_Line/main/0000.C01".to_string(),
            _ => panic!("Unsupported game type"),
        }
    }

    fn load_index(vfs: &MiniFs) -> anyhow::Result<Vec<Option<AtpEntry>>> {
        let mut entries = vec![];
        for i in 1..99 {
            let path = format!("/ACT/{:0>8}.atp", i);
            if vfs.open(&path).is_err() {
                continue;
            }

            let content = vfs.read_to_end(path)?;
            let atp = AtpFile::read(&content)?;
            entries.extend(atp.files);
        }

        Ok(entries)
    }
}
