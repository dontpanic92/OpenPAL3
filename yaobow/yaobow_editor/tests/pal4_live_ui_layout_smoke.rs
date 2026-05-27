//! Live PAL4 smoke for the editor's `UiLayoutHandle::try_create`
//! against the production CPK-backed vfs (`packfs::init_virtual_fs`).
//! Catches host-side regressions that pure shared-loader smokes miss
//! (path translation through CpkFs, ImagesetIndex dir-scan fallback,
//! texture-cache upload).
//!
//! Skipped automatically when neither known install path is present.
//!
//! Activate verbose tracing by setting `RUST_LOG=info` and using
//! `cargo test -p yaobow_editor --test pal4_live_ui_layout_smoke
//! -- --nocapture`.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use imgui::TextureId;
use packfs::init_virtual_fs;
use radiance::rendering::{
    ComponentFactory, MaterialDef, RenderObjectHandle, RenderingComponent, Texture, TextureDef,
    VertexBuffer, VideoPlayer,
};
use radiance_scripting::services::ImguiTextureCache;
use yaobow_editor::comdef::editor_services::IUiLayoutHandle;
use yaobow_editor::services::handles::UiLayoutHandle;

struct DummyTexture;
impl Texture for DummyTexture {
    fn width(&self) -> u32 {
        1
    }
    fn height(&self) -> u32 {
        1
    }
}

struct MockFactory {
    uploads: Cell<usize>,
}

impl ComponentFactory for MockFactory {
    fn create_texture(&self, _: &TextureDef) -> Box<dyn Texture> {
        Box::new(DummyTexture)
    }
    fn create_imgui_texture(
        &self,
        _: &[u8],
        _: u32,
        _: u32,
        _: u32,
        _: Option<TextureId>,
    ) -> (Box<dyn Texture>, TextureId) {
        let n = self.uploads.get() + 1;
        self.uploads.set(n);
        (Box::new(DummyTexture), TextureId::new(n))
    }
    fn remove_imgui_texture(&self, _: Option<TextureId>) {}
    fn create_render_object(
        &self,
        _: VertexBuffer,
        _: Vec<u32>,
        _: &MaterialDef,
        _: bool,
    ) -> RenderObjectHandle {
        unimplemented!()
    }
    fn create_rendering_component(&self, _: Vec<RenderObjectHandle>) -> RenderingComponent {
        RenderingComponent::new()
    }
    fn create_video_player(&self) -> Box<VideoPlayer> {
        Box::new(VideoPlayer::new())
    }
    fn create_render_target(
        &self,
        _: u32,
        _: u32,
    ) -> Box<dyn radiance::rendering::RenderTarget> {
        unimplemented!()
    }
}

fn pal4_root() -> Option<PathBuf> {
    for cand in [
        r"F:\SteamLibrary\steamapps\common\Chinese Paladin 4",
        r"F:\pal4",
    ] {
        let p = PathBuf::from(cand);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

fn try_layout(vfs: &mini_fs::MiniFs) -> Option<ComRc<IUiLayoutHandle>> {
    // Try both possible layout paths in priority order: the CPK-backed
    // `gamedata/ui/layouts/` (Steam + standard install) wins because
    // `ui.cpk` is mounted at `gamedata/ui/`.
    let layout_paths = [
        "/gamedata/ui/layouts/CombatMainWindow.xml",
        // Legacy fallback for the ad-hoc dev extract used during early
        // M1 testing; the production code only emits the canonical
        // path today.
        "/gamedata/ui2/ui/layouts/CombatMainWindow.xml",
    ];
    let factory: Rc<dyn ComponentFactory> = Rc::new(MockFactory {
        uploads: Cell::new(0),
    });
    let cache = Rc::new(RefCell::new(ImguiTextureCache::new(factory)));
    for p in &layout_paths {
        eprintln!("=== attempting layout: {} ===", p);
        if let Some(h) = UiLayoutHandle::try_create(vfs, p, cache.clone()) {
            return Some(h);
        }
    }
    None
}

#[test]
fn live_pal4_combat_main_window_produces_draws() {
    // Best-effort logger init so `RUST_LOG=info` works.
    let _ = env_logger::builder().is_test(true).try_init();

    let Some(root) = pal4_root() else {
        eprintln!("no PAL4 install found — skipping");
        return;
    };
    eprintln!("PAL4 root: {}", root.display());
    let vfs = init_virtual_fs(&root, None);

    let h = try_layout(&vfs).expect("UiLayoutHandle should construct against a real PAL4 install");

    let nw = h.native_width();
    let nh = h.native_height();
    let dc = h.draw_count();
    let wc = h.window_count();
    eprintln!(
        "handle: native={}x{}, draws={}, windows={}",
        nw, nh, dc, wc
    );
    assert!(nw > 0 && nh > 0);
    assert!(wc > 0);
    assert!(
        dc > 0,
        "expected non-zero draw count, got {} (every imageset resolution likely failed; check the info-level logs)",
        dc
    );

    // Sanity-check the first few draws so we'd catch a regression
    // where everything is uploaded but every cmd has zero-size or
    // collapsed UVs (which would produce an empty preview at
    // runtime).
    let mut bad_dims = 0;
    let mut bad_uv = 0;
    let mut bad_tex = 0;
    for i in 0..dc {
        let w = h.draw_w(i);
        let hgt = h.draw_h(i);
        let u0 = h.draw_u0(i);
        let v0 = h.draw_v0(i);
        let u1 = h.draw_u1(i);
        let v1 = h.draw_v1(i);
        let tex = h.draw_texture_com_id(i);
        if w <= 0.0 || hgt <= 0.0 {
            bad_dims += 1;
        }
        if (u1 - u0).abs() < f32::EPSILON || (v1 - v0).abs() < f32::EPSILON {
            bad_uv += 1;
        }
        if tex == 0 {
            bad_tex += 1;
        }
        if i < 5 {
            eprintln!(
                "draw[{}]: ({:.1},{:.1}) {:.1}x{:.1} tex={} uv=({:.3},{:.3})..({:.3},{:.3}) interactive={} text={:?}",
                i,
                h.draw_x(i),
                h.draw_y(i),
                w,
                hgt,
                tex,
                u0,
                v0,
                u1,
                v1,
                h.draw_is_interactive(i),
                h.draw_text(i),
            );
        }
    }
    assert_eq!(bad_dims, 0, "{} draw cmds with zero dims", bad_dims);
    assert_eq!(bad_uv, 0, "{} draw cmds with collapsed UVs", bad_uv);
    assert_eq!(bad_tex, 0, "{} draw cmds with no texture", bad_tex);
}
