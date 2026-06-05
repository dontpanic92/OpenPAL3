//! `ISceneHandle` / `IScenePreviewSession` / `IInspectorView` Rust impls.
//!
//! Wraps a loaded `Pal4Scene` (PAL4-only in v1) and exposes a flat
//! node-id tree to the outline panel plus an on-demand `InspectorView`
//! for the currently-selected node. The preview session shares the
//! `PreviewRegistry` / `OrbitState` machinery used by the existing
//! single-model `PreviewSession`, so `host.render_pending_previews()`
//! drives both transparently.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IEntity, IEntityExt, IScene, ISceneExt};
use radiance::math::{Vec3, transform_aabb};
use radiance::rendering::ComponentFactory;
use radiance_scripting::comdef::services::IRenderTarget;
use radiance_scripting::services::{ImguiTextureCache, ScriptedRenderTarget};

use crate::comdef::editor_services::{
    IInspectorView, IInspectorViewImpl, ISceneHandle, ISceneHandleImpl, IScenePreviewSession,
    IScenePreviewSessionImpl,
};
use crate::services::gizmo::Gizmo;
use crate::services::preview_registry::{OrbitState, PreviewRegistry, PreviewState, tick_orbit};
use shared::openpal4::asset_loader::AssetLoader as Pal4AssetLoader;

/// Flat-node-tree representation of the loaded scene. `nodes[i].parent`
/// is `-1` for root entities; otherwise it indexes into the same `Vec`.
pub struct SceneNode {
    pub name: String,
    pub parent: i32,
    pub kind: i32,
    pub entity: ComRc<IEntity>,
}

pub struct SceneHandle {
    nodes: RefCell<Vec<SceneNode>>,
    scene: RefCell<Option<ComRc<IScene>>>,
    factory: Rc<dyn ComponentFactory>,
    cache: Rc<RefCell<ImguiTextureCache>>,
    registry: Rc<PreviewRegistry>,
    last_string: RefCell<String>,
}

ComObject_SceneHandle!(super::SceneHandle);

impl SceneHandle {
    /// Try to interpret `vfs_path` as a PAL4 scene-block reference and
    /// load it via `Pal4Scene::load`. Returns `None` when the path
    /// doesn't match the `/gamedata/PALWorld/<scene>/<block>/<block>.bsp`
    /// convention, when the asset loader isn't a PAL4 loader, or when
    /// loading fails with a recoverable error (e.g. missing BSP, parse
    /// failure, missing sibling asset). Loader-side I/O is now fully
    /// `Result`-based, so a misclicked BSP propagates a normal error
    /// up here without ever tripping the host's panic hook.
    pub fn try_create_pal4(
        vfs_path: &str,
        loader: &Rc<Pal4AssetLoader>,
        input: Rc<RefCell<dyn radiance::input::InputEngine>>,
        factory: Rc<dyn ComponentFactory>,
        cache: Rc<RefCell<ImguiTextureCache>>,
        registry: Rc<PreviewRegistry>,
    ) -> Option<ComRc<ISceneHandle>> {
        let (scene_name, block_name) = parse_pal4_scene_path(vfs_path)?;

        use shared::openpal4::scene::Pal4Scene;
        let pal4_scene = match Pal4Scene::load(loader, input, &scene_name, &block_name, None) {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "SceneHandle: failed to load PAL4 scene {}/{}: {:#}",
                    scene_name,
                    block_name,
                    e
                );
                return None;
            }
        };

        // `Pal4Scene` keeps several private fields, but the underlying
        // `ComRc<IScene>` is reachable via `into_scene()` if exposed —
        // otherwise we drop the wrapper and only keep the IScene we
        // can reach. The wrapper's other fields (events, gob, etc.)
        // are gameplay-only and not needed for a read-only preview.
        let scene = pal4_scene_take_scene(pal4_scene);

        // `Pal4Scene::load` adds entities to the scene while the scene
        // itself is unloaded, so `IScene::add_entity` short-circuits
        // the per-entity `load()` call that fires `on_loading` on
        // each component. Without that, the rendering component never
        // initializes its backend render objects and the preview is
        // an empty black target. The gameplay path normally relies on
        // `ISceneManager::set_scene` to call `scene.load()` once it
        // becomes active; for our offscreen path we drive it here.
        scene.load();

        let mut nodes: Vec<SceneNode> = Vec::new();
        for root in scene.root_entities() {
            collect_nodes(&root, -1, &mut nodes);
        }

        Some(ComRc::from_object(Self {
            nodes: RefCell::new(nodes),
            scene: RefCell::new(Some(scene)),
            factory,
            cache,
            registry,
            last_string: RefCell::new(String::new()),
        }))
    }

    fn set_last(&self, s: String) -> &str {
        let s = if s.contains('\0') {
            s.replace('\0', "")
        } else {
            s
        };
        *self.last_string.borrow_mut() = s;
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
}

fn collect_nodes(entity: &ComRc<IEntity>, parent: i32, out: &mut Vec<SceneNode>) {
    let idx = out.len() as i32;
    let name = entity.name();
    let kind = if parent < 0 { 0 } else { 1 };
    out.push(SceneNode {
        name,
        parent,
        kind,
        entity: entity.clone(),
    });
    for child in entity.children() {
        collect_nodes(&child, idx, out);
    }
}

/// Extract the underlying `IScene` from a `Pal4Scene`. The wrapper's
/// `scene` field is `pub(crate)`, so we go through the `IScene` clone
/// path via the manager: load the scene, then mirror its
/// `add_entity` semantics through a fresh root the editor owns. For
/// v1 we keep it simple and *transmute* via a small accessor module —
/// see the helper below.
fn pal4_scene_take_scene(scene: shared::openpal4::scene::Pal4Scene) -> ComRc<IScene> {
    // We deliberately drop the rest of `Pal4Scene`. The `IScene` ref-
    // count is preserved through the accessor.
    shared::openpal4::scene_editor_access::take_scene(scene)
}

fn parse_pal4_scene_path(vfs_path: &str) -> Option<(String, String)> {
    // PAL4's `AssetLoader::load_scene` rebuilds the BSP path as
    // `/gamedata/PALWorld/<scene>/<block>/<block>.bsp`. We accept any
    // `.bsp` path that fits that layout (case-insensitive PALWorld
    // segment), and require the filename stem to match the block name
    // so we don't try to load, say, a sub-mesh BSP that happens to
    // live under the scene directory.
    let ext = std::path::Path::new(vfs_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    if ext.as_deref() != Some("bsp") {
        return None;
    }
    let trimmed = vfs_path.trim_start_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    // Expected: ["gamedata", "PALWorld", "<scene>", "<block>", "<block>.bsp"]
    if parts.len() < 5 {
        return None;
    }
    if !parts[0].eq_ignore_ascii_case("gamedata") {
        return None;
    }
    if !parts[1].eq_ignore_ascii_case("palworld") {
        return None;
    }
    let scene = parts[2].to_string();
    let block = parts[3].to_string();
    let file_stem = std::path::Path::new(parts[4])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if scene.is_empty() || block.is_empty() {
        return None;
    }
    if !file_stem.eq_ignore_ascii_case(&block) {
        return None;
    }
    Some((scene, block))
}

/// Walk the scene's render objects, union their world-space AABBs and
/// return a (focus, distance) pair suitable for seeding `OrbitState`.
/// Falls back to (origin, 200) when the scene exposes no AABB info
/// (typical for purely-procedural or empty scenes).
fn compute_initial_framing(scene: &ComRc<IScene>) -> (Vec3, f32) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut aabb_count = 0usize;
    let mut entity_count = 0usize;
    let mut pos_min = [f32::INFINITY; 3];
    let mut pos_max = [f32::NEG_INFINITY; 3];

    for root in scene.root_entities() {
        accumulate_entity_aabb(
            &root,
            &mut min,
            &mut max,
            &mut aabb_count,
            &mut entity_count,
            &mut pos_min,
            &mut pos_max,
        );
    }

    log::info!(
        "SceneHandle: framing scan — entities={}, render-object AABBs={}",
        entity_count,
        aabb_count
    );

    // Prefer the per-render-object AABB union when available. When no
    // render object reports bounds (vitagl backend or freshly-loaded
    // entities), fall back to the union of world-transform positions
    // so we at least frame *something* instead of dropping back to
    // origin (where the scene is rarely centred).
    let (min, max) = if aabb_count > 0 {
        log::info!("SceneHandle: AABB union min={:?} max={:?}", min, max);
        (min, max)
    } else if entity_count > 0 {
        log::info!(
            "SceneHandle: AABB unavailable, framing from entity positions min={:?} max={:?}",
            pos_min,
            pos_max
        );
        (pos_min, pos_max)
    } else {
        log::warn!("SceneHandle: no entities found — defaulting to origin framing");
        return (Vec3::new(0., 0., 0.), 200.0);
    };

    let cx = 0.5 * (min[0] + max[0]);
    let cy = 0.5 * (min[1] + max[1]);
    let cz = 0.5 * (min[2] + max[2]);
    let dx = (max[0] - min[0]).max(0.0);
    let dy = (max[1] - min[1]).max(0.0);
    let dz = (max[2] - min[2]).max(0.0);
    let diag = (dx * dx + dy * dy + dz * dz).sqrt();
    // Pad the diagonal so the camera frames the whole scene with room
    // around the edges.
    let distance = (diag * 1.5).max(200.0);
    log::info!(
        "SceneHandle: initial framing focus=({:.1},{:.1},{:.1}) diag={:.1} distance={:.1}",
        cx,
        cy,
        cz,
        diag,
        distance
    );
    (Vec3::new(cx, cy, cz), distance)
}

fn accumulate_entity_aabb(
    entity: &ComRc<IEntity>,
    min: &mut [f32; 3],
    max: &mut [f32; 3],
    aabb_count: &mut usize,
    entity_count: &mut usize,
    pos_min: &mut [f32; 3],
    pos_max: &mut [f32; 3],
) {
    *entity_count += 1;
    let world = entity.world_transform();
    let pos = world.position();
    for (i, v) in [pos.x, pos.y, pos.z].iter().enumerate() {
        if *v < pos_min[i] {
            pos_min[i] = *v;
        }
        if *v > pos_max[i] {
            pos_max[i] = *v;
        }
    }
    if let Some(rc) = entity.get_rendering_component() {
        let m = world.matrix();
        for ro in rc.render_objects() {
            if let Some((lmin, lmax)) = ro.as_dyn().local_aabb() {
                let (wmin, wmax) = transform_aabb(lmin, lmax, m);
                for i in 0..3 {
                    if wmin[i] < min[i] {
                        min[i] = wmin[i];
                    }
                    if wmax[i] > max[i] {
                        max[i] = wmax[i];
                    }
                }
                *aabb_count += 1;
            }
        }
    }
    for child in entity.children() {
        accumulate_entity_aabb(&child, min, max, aabb_count, entity_count, pos_min, pos_max);
    }
}

impl ISceneHandleImpl for SceneHandle {
    fn open_preview(&self) -> ComRc<IScenePreviewSession> {
        let scene = self
            .scene
            .borrow_mut()
            .take()
            .expect("SceneHandle::open_preview can only be called once");

        // Offscreen target — the script will resize() it to the
        // viewport's available region every frame, just like the
        // single-model preview does.
        let target_box = self.factory.create_render_target(640, 480);
        let (target_com, target_shared) =
            ScriptedRenderTarget::create(target_box, self.cache.clone());

        // Spawn the selection gizmo before the scene goes into the
        // preview state — Gizmo::create attaches its entities to the
        // scene so they ride the same offscreen pass and orbit camera.
        let gizmo = Gizmo::create(&scene, self.factory.clone());

        // Compute a sensible initial orbit framing: walk the scene's
        // render objects, union their world-space AABBs, then place
        // the orbit camera at a distance that fits the diagonal in
        // the camera's FOV. Without this the camera sits at the
        // world origin (default) while a PAL4 BSP lives several
        // thousand world units away — producing a black viewport.
        let (focus, distance) = compute_initial_framing(&scene);

        let orbit = OrbitState::new(focus, 0.6, 0.4, distance);
        let state = Rc::new(PreviewState {
            scene,
            target: target_shared,
            orbit: RefCell::new(orbit),
            closed: Cell::new(false),
        });
        // Bump near/far so a far-framed scene doesn't get clipped.
        // `Camera::new_with_params` defaults are `near=10, far=100000`;
        // PAL4 scenes can be ~5k units across, so the default far is
        // fine but the near plane can poke through nearby geometry
        // when the orbit zooms in.
        state.apply_camera();
        self.registry.register(&state);

        ScenePreviewSession::create(
            state,
            target_com,
            self.nodes.borrow().clone_for_view(),
            gizmo,
        )
    }

    fn node_count(&self) -> i32 {
        self.nodes.borrow().len() as i32
    }

    fn node_parent(&self, id: i32) -> i32 {
        self.nodes
            .borrow()
            .get(id as usize)
            .map(|n| n.parent)
            .unwrap_or(-1)
    }

    fn node_name(&self, id: i32) -> &str {
        let name = self
            .nodes
            .borrow()
            .get(id as usize)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        self.set_last(name)
    }

    fn node_kind(&self, id: i32) -> i32 {
        self.nodes
            .borrow()
            .get(id as usize)
            .map(|n| n.kind)
            .unwrap_or(0)
    }
}

/// View of the node tree shared with the preview session. We can't
/// hand out the `RefCell` directly without inviting borrow cycles, so
/// we clone the (name, parent, kind) triplets and keep weak entity
/// pointers via the original ComRc — entities are kept alive by the
/// preview scene.
pub struct NodeView {
    pub name: String,
    pub parent: i32,
    pub kind: i32,
    pub entity: ComRc<IEntity>,
}

trait CloneForView {
    fn clone_for_view(&self) -> Vec<NodeView>;
}

impl CloneForView for Vec<SceneNode> {
    fn clone_for_view(&self) -> Vec<NodeView> {
        self.iter()
            .map(|n| NodeView {
                name: n.name.clone(),
                parent: n.parent,
                kind: n.kind,
                entity: n.entity.clone(),
            })
            .collect()
    }
}

// -------------------------------------------------------------------
// ScenePreviewSession
// -------------------------------------------------------------------

pub struct ScenePreviewSession {
    state: Rc<PreviewState>,
    target_com: ComRc<IRenderTarget>,
    nodes: Vec<NodeView>,
    selection: Cell<i32>,
    gizmo_axes: Cell<bool>,
    gizmo_aabb: Cell<bool>,
    gizmo_pivot: Cell<bool>,
    gizmo: Gizmo,
}

ComObject_ScenePreviewSession!(super::ScenePreviewSession);

impl ScenePreviewSession {
    pub fn create(
        state: Rc<PreviewState>,
        target_com: ComRc<IRenderTarget>,
        nodes: Vec<NodeView>,
        gizmo: Gizmo,
    ) -> ComRc<IScenePreviewSession> {
        ComRc::from_object(Self {
            state,
            target_com,
            nodes,
            selection: Cell::new(-1),
            gizmo_axes: Cell::new(true),
            gizmo_aabb: Cell::new(true),
            gizmo_pivot: Cell::new(true),
            gizmo,
        })
    }

    fn refresh_gizmo(&self) {
        let id = self.selection.get();
        let world_pos = self
            .nodes
            .get(id as usize)
            .map(|n| n.entity.world_transform().position());
        self.gizmo.update(
            world_pos,
            self.gizmo_axes.get(),
            self.gizmo_aabb.get(),
            self.gizmo_pivot.get(),
        );
    }
}

impl Drop for ScenePreviewSession {
    fn drop(&mut self) {
        self.state.closed.set(true);
    }
}

impl IScenePreviewSessionImpl for ScenePreviewSession {
    fn close(&self) {
        self.state.closed.set(true);
    }

    fn target(&self) -> ComRc<IRenderTarget> {
        self.target_com.clone()
    }

    fn tick_camera(&self, dx: f32, dy: f32, wheel: f32, buttons: i32) {
        if self.state.closed.get() {
            return;
        }
        let mut orbit = self.state.orbit.borrow_mut();
        tick_orbit(&mut orbit, dx, dy, wheel, buttons);
    }

    fn set_selection(&self, node_id: i32) {
        self.selection.set(node_id);
        self.refresh_gizmo();
    }

    fn selection(&self) -> i32 {
        self.selection.get()
    }

    fn set_gizmo_visible(&self, axes: i32, aabb: i32, pivot: i32) {
        self.gizmo_axes.set(axes != 0);
        self.gizmo_aabb.set(aabb != 0);
        self.gizmo_pivot.set(pivot != 0);
        self.refresh_gizmo();
    }

    fn inspector_for(&self, node_id: i32) -> ComRc<IInspectorView> {
        InspectorView::create(self.nodes.get(node_id as usize))
    }
}

// -------------------------------------------------------------------
// InspectorView
// -------------------------------------------------------------------

pub struct InspectorView {
    fields: Vec<(String, String)>,
    last_string: RefCell<String>,
}

ComObject_InspectorView!(super::InspectorView);

impl InspectorView {
    pub fn create(node: Option<&NodeView>) -> ComRc<IInspectorView> {
        let fields = match node {
            None => vec![("(no selection)".to_string(), "".to_string())],
            Some(n) => {
                let mut f = Vec::with_capacity(8);
                f.push(("name".to_string(), n.name.clone()));
                f.push(("kind".to_string(), kind_label(n.kind).to_string()));
                f.push(("parent".to_string(), n.parent.to_string()));
                f.push((
                    "children".to_string(),
                    n.entity.children().len().to_string(),
                ));
                let transform = n.entity.transform();
                let t = transform.borrow();
                let p = t.position();
                let e = t.euler();
                f.push((
                    "position".to_string(),
                    format!("{:.3}, {:.3}, {:.3}", p.x, p.y, p.z),
                ));
                f.push((
                    "rotation_euler_deg".to_string(),
                    format!("{:.1}, {:.1}, {:.1}", e.x, e.y, e.z),
                ));
                f.push(("visible".to_string(), n.entity.visible().to_string()));
                f
            }
        };
        ComRc::from_object(Self {
            fields,
            last_string: RefCell::new(String::new()),
        })
    }

    fn set_last(&self, s: String) -> &str {
        let s = if s.contains('\0') {
            s.replace('\0', "")
        } else {
            s
        };
        *self.last_string.borrow_mut() = s;
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
}

fn kind_label(kind: i32) -> &'static str {
    match kind {
        0 => "root",
        1 => "child",
        2 => "actor",
        3 => "skybox",
        4 => "water",
        5 => "floor",
        6 => "wall",
        7 => "npc",
        _ => "?",
    }
}

impl IInspectorViewImpl for InspectorView {
    fn field_count(&self) -> i32 {
        self.fields.len() as i32
    }

    fn field_key(&self, i: i32) -> &str {
        let v = self
            .fields
            .get(i as usize)
            .map(|(k, _)| k.clone())
            .unwrap_or_default();
        self.set_last(v)
    }

    fn field_value(&self, i: i32) -> &str {
        let v = self
            .fields
            .get(i as usize)
            .map(|(_, val)| val.clone())
            .unwrap_or_default();
        self.set_last(v)
    }
}
