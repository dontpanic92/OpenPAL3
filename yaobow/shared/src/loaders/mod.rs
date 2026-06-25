use std::{
    ffi::OsStr,
    io::Read,
    path::{Path, PathBuf},
};

use common::store_ext::StoreExt2;
use mini_fs::MiniFs;

pub mod anm;
pub mod bsp;
pub mod cegui;
pub mod dff;
pub mod smp;
pub mod video_handle;

pub trait TextureResolver: Send + Sync {
    fn resolve_texture(
        &self,
        vfs: &MiniFs,
        model_path: &Path,
        texture_name: &str,
    ) -> Option<Vec<u8>>;
}

/// A leaf / sprite card resolved from PAL5's `Config/uvlist.tb` for a model's
/// `[W]/[w]{t<id>}` foliage quad: the atlas texture (relative to the `Texture`
/// root, backslash-separated as shipped — e.g. `BuildingP5\zhiwu\tree_yinxingqiu`)
/// and the atlas-space UV rectangle `(u0, u1, v0, v1)` to map onto the quad.
#[derive(Debug, Clone)]
pub struct FoliageCard {
    pub atlas: String,
    pub uv: [f32; 4],
}

/// Resolves a PAL5 `{t<id>}` foliage-card id to its [`FoliageCard`]. PAL5 ships
/// many tree-leaf quads with no texture and no UVs in the model; the engine
/// supplies both at load time from `uvlist.tb`, keyed by the `{t<id>}` tag. See
/// `generated/pal5_leaf_re.md`.
pub trait FoliageResolver: Send + Sync {
    fn resolve_card(&self, id: u32) -> Option<FoliageCard>;
}

pub struct Pal4TextureResolver;
impl TextureResolver for Pal4TextureResolver {
    fn resolve_texture(
        &self,
        vfs: &MiniFs,
        model_path: &Path,
        texture_name: &str,
    ) -> Option<Vec<u8>> {
        let tex_path = model_path
            .parent()
            .unwrap()
            .join(texture_name.to_string() + ".dds");

        let mut data = vec![];
        let _ = vfs
            .open_with_fallback(&tex_path, &["png"])
            .ok()?
            .read_to_end(&mut data)
            .ok()?;

        Some(data)
    }
}

pub struct Pal5TextureResolver;
impl TextureResolver for Pal5TextureResolver {
    fn resolve_texture(
        &self,
        vfs: &MiniFs,
        model_path: &Path,
        texture_name: &str,
    ) -> Option<Vec<u8>> {
        let candidates = pal5_texture_candidates(model_path, texture_name);

        let mut data = vec![];
        let _ = vfs
            .try_open_files(&candidates)
            .ok()?
            .read_to_end(&mut data)
            .ok()?;

        Some(data)
    }
}

/// Builds the ordered list of VFS paths to probe for a PAL5 texture
/// referenced by `texture_name` from a DFF at `model_path`.
///
/// PAL5's `role_*.bin` ships asset `file_path` entries with Windows
/// backslash separators (e.g. `BuildingP5\jianzhu\foo.dff`), so the
/// `model_path` we receive here is typically of the form
/// `/Model/BuildingP5\jianzhu\foo.dff` — a mix of `/` and `\` that
/// breaks `Path::parent()` / `.iter()` / `.file_name()` on Unix (where
/// `\` is a literal character, not a separator). Models still load
/// because `packfs::pkg::pkg_archive::open` re-normalises `/` → `\`
/// before lookup, but the texture resolver does its own path math and
/// must canonicalise first.
///
/// The helper:
/// 1. Normalises every `\` in `model_path` to `/`, giving a clean
///    POSIX `PathBuf` for the subsequent operations.
/// 2. Short-circuits on the special `jiemian.dff` UI model to
///    `/Texture/load/xianjianwu/<name>.dds`.
/// 3. Otherwise strips the leading `/Model` segment (`iter().skip(2)`
///    drops the root `/` and the `Model` component) and re-roots the
///    rest under `/Texture`, appending `<name>.dds`.
/// 4. For `jianzhu` models adds two sibling-directory fallbacks
///    (`ZhuangShi/` and `jianzhu/ssd/`) that real PAL5 building
///    assets occasionally reach across.
///
/// Pkg lookups are slash-agnostic (`pkg_archive::open` re-normalises
/// `/` → `\`), so the returned forward-slash paths work uniformly
/// against both `LocalFs` and `PkgFs` mounts.
fn pal5_texture_candidates(model_path: &Path, texture_name: &str) -> Vec<PathBuf> {
    // Foliage cards (`uvlist.tb`) reference their atlas by a full
    // `Texture`-root-relative path (e.g. `BuildingP5\zhiwu\tree_yinxingqiu`)
    // rather than a bare RW texture name. Detect that by the presence of a
    // path separator and resolve straight under `/Texture`. Ordinary RW
    // material textures are bare identifiers, so this never affects them.
    if texture_name.contains('\\') || texture_name.contains('/') {
        let norm = texture_name.replace('\\', "/");
        return vec![PathBuf::from(format!("/Texture/{}.dds", norm))];
    }

    let normalised = PathBuf::from(model_path.to_string_lossy().replace('\\', "/"));

    if normalised.file_name() == Some(OsStr::new("jiemian.dff")) {
        return vec![PathBuf::from(format!(
            "/Texture/load/xianjianwu/{}.dds",
            texture_name
        ))];
    }

    let mut paths = vec![];

    let relative_path: PathBuf = normalised
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .iter()
        .skip(2)
        .collect();
    let tex_path = PathBuf::from("/Texture")
        .join(relative_path)
        .join(texture_name.to_string() + ".dds");

    paths.push(tex_path.clone());

    if normalised
        .to_str()
        .map(|s| s.contains("jianzhu"))
        .unwrap_or(false)
    {
        if let Some(building_path) = tex_path.parent().and_then(Path::parent) {
            paths.push(
                building_path
                    .join("ZhuangShi")
                    .join(texture_name.to_string() + ".dds"),
            );
            paths.push(
                building_path
                    .join("jianzhu")
                    .join("ssd")
                    .join(texture_name.to_string() + ".dds"),
            );
        }
    }

    paths
}

pub struct Swd5TextureResolver;
impl TextureResolver for Swd5TextureResolver {
    fn resolve_texture(&self, vfs: &MiniFs, _: &Path, texture_name: &str) -> Option<Vec<u8>> {
        let tex_path = PathBuf::from("/Texture/texture").join(texture_name.to_string() + ".png");

        let mut data = vec![];
        let _ = vfs
            .open_with_fallback(&tex_path, &["dds"])
            .ok()?
            .read_to_end(&mut data)
            .ok()?;

        Some(data)
    }
}

#[cfg(test)]
mod pal5_texture_tests {
    use super::pal5_texture_candidates;
    use std::path::{Path, PathBuf};

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn backslash_model_path_resolves_under_building() {
        // role.bin stores `BuildingP5\jianzhu\jz_*.dff`; the resolver
        // must reach `/Texture/BuildingP5/jianzhu/<name>.dds` and the
        // two jianzhu fallbacks under `/Texture/BuildingP5/`.
        let got = pal5_texture_candidates(
            Path::new("/Model/BuildingP5\\jianzhu\\jz_shanzhaiqiang_A.dff"),
            "jz_szmu_D",
        );
        assert_eq!(
            got,
            vec![
                p("/Texture/BuildingP5/jianzhu/jz_szmu_D.dds"),
                p("/Texture/BuildingP5/ZhuangShi/jz_szmu_D.dds"),
                p("/Texture/BuildingP5/jianzhu/ssd/jz_szmu_D.dds"),
            ],
            "backslash separators must produce the same candidates as forward slashes"
        );
    }

    #[test]
    fn forward_slash_model_path_matches_backslash_output() {
        let got = pal5_texture_candidates(
            Path::new("/Model/BuildingP5/jianzhu/jz_shanzhaiqiang_A.dff"),
            "jz_szmu_D",
        );
        assert_eq!(
            got,
            vec![
                p("/Texture/BuildingP5/jianzhu/jz_szmu_D.dds"),
                p("/Texture/BuildingP5/ZhuangShi/jz_szmu_D.dds"),
                p("/Texture/BuildingP5/jianzhu/ssd/jz_szmu_D.dds"),
            ],
            "forward-slash paths must produce the canonical candidate list"
        );
    }

    #[test]
    fn non_jianzhu_model_skips_fallbacks() {
        // `zhiwu/zw_*.dff` has only the primary `/Texture/<rel>/<name>.dds`
        // candidate — no `ZhuangShi` / `ssd` fallbacks.
        let got = pal5_texture_candidates(
            Path::new("/Model/BuildingP5\\zhiwu\\zw_shulin_07.dff"),
            "zw_shugan02",
        );
        assert_eq!(got, vec![p("/Texture/BuildingP5/zhiwu/zw_shugan02.dds")]);
    }

    #[test]
    fn jiemian_dff_uses_xianjianwu_short_circuit() {
        // The UI front-end model maps to `/Texture/load/xianjianwu/`
        // regardless of where the `jiemian.dff` lives.
        let backslash =
            pal5_texture_candidates(Path::new("/Model/UI\\frontend\\jiemian.dff"), "title_logo");
        let forward =
            pal5_texture_candidates(Path::new("/Model/UI/frontend/jiemian.dff"), "title_logo");
        let expected = vec![p("/Texture/load/xianjianwu/title_logo.dds")];
        assert_eq!(backslash, expected);
        assert_eq!(forward, expected);
    }

    #[test]
    fn foliage_atlas_path_resolves_under_texture_root() {
        // A `uvlist.tb` leaf-card atlas is a full Texture-root-relative path
        // (contains separators), so it resolves directly under `/Texture`,
        // independent of the model's directory.
        let got = pal5_texture_candidates(
            Path::new("/Model/BuildingP5\\zhiwu\\zw_shulin_07.dff"),
            "BuildingP5\\zhiwu\\tree_yinxingqiu",
        );
        assert_eq!(
            got,
            vec![p("/Texture/BuildingP5/zhiwu/tree_yinxingqiu.dds")]
        );
    }
}
