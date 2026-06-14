use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use crosscom::ComRc;
use serde::Serialize;

use crate::{
    comdef::{
        IAnimationEventObserver, IArmatureComponent, IArmatureComponentImpl, IComponentImpl,
        IEntity, IHAnimBoneComponent, IHAnimBoneComponentImpl, ISkinnedMeshComponentImpl,
    },
    math::{Mat44, Quaternion, Transform, Vec3},
    rendering::{ComponentFactory, VertexBuffer, VertexComponents},
};

use super::{
    Geometry,
    event::{AnimationEvent, AnimationEventManager},
};
use crate::comdef::IEntityExt;

pub struct SkinnedMeshComponent {
    entity: ComRc<IEntity>,
    component_factory: Rc<dyn ComponentFactory>,
    geometry: Geometry,
    armature: ComRc<IArmatureComponent>,
    bones: Vec<ComRc<IEntity>>,
    bone_components: Vec<ComRc<IHAnimBoneComponent>>,
    v_bone_id: Vec<[usize; 4]>,
    v_weights: Vec<[f32; 4]>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AnimationState {
    NoAnimation,
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct AnimKeyFrame {
    pub rotation: Quaternion,
    pub position: Vec3,
    pub timestamp: f32,
}

impl SkinnedMeshComponent {
    pub fn new(
        entity: ComRc<IEntity>,
        component_factory: Rc<dyn ComponentFactory>,
        geometry: Geometry,
        armature: ComRc<IArmatureComponent>,
        v_bone_id: Vec<[usize; 4]>,
        v_weights: Vec<[f32; 4]>,
    ) -> Self {
        let bones = armature.bones();
        let bone_components = bones
            .iter()
            .map(|b| {
                b.get_component(IHAnimBoneComponent::uuid())
                    .unwrap()
                    .query_interface::<IHAnimBoneComponent>()
                    .unwrap()
            })
            .collect();

        Self {
            entity,
            component_factory,
            geometry,
            armature,
            bones,
            bone_components,
            v_bone_id,
            v_weights,
        }
    }

    fn load_geometries(&self) {
        let mut objects = vec![];

        let ro = self.component_factory.create_render_object(
            self.geometry.vertices.clone(),
            self.geometry.indices.clone(),
            &self.geometry.material,
            true,
        );

        objects.push(ro);

        let component = self.component_factory.create_rendering_component(objects);
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn update_vertex_buffer(&self, mut vertex_buffer: RefMut<VertexBuffer>) {
        let use_bond_pose = self.armature.animation_state() == AnimationState::NoAnimation;

        if use_bond_pose {
            // No animation: copy the original mesh-space positions through
            // unchanged. The geometry buffer already holds these on initial
            // upload, but `on_updating` re-runs this function every tick, so
            // restore them explicitly in case a prior animation left
            // deformed positions in the buffer.
            for i in 0..vertex_buffer.count() {
                let v = *self.geometry.vertices.position(i).unwrap();
                vertex_buffer.set_component(i, VertexComponents::POSITION, |p: &mut Vec3| {
                    *p = v;
                });
            }
            return;
        }

        // Precompute `skin = bond_pose * world_transform` for each bone
        // once per frame. This is the linear-blend-skinning matrix applied
        // to mesh-space vertices.
        let skin_mats: Vec<Mat44> = self
            .bone_components
            .iter()
            .zip(self.bones.iter())
            .map(|(bc, bone)| {
                let bond_pose = bc.bond_pose();
                let world = bone.world_transform().matrix().clone();
                Mat44::multiplied(&world, &bond_pose)
            })
            .collect();

        for i in 0..vertex_buffer.count() {
            let v = self.geometry.vertices.position(i).unwrap();
            let ids = &self.v_bone_id[i];
            let weights = &self.v_weights[i];

            let mut blended = Vec3::new(0., 0., 0.);
            let mut total_weight = 0.0f32;
            for k in 0..4 {
                let w = weights[k];
                if w == 0.0 {
                    continue;
                }
                let bone_id = ids[k];
                if bone_id >= skin_mats.len() {
                    continue;
                }
                let contrib = Vec3::crossed_mat(v, &skin_mats[bone_id]);
                blended = Vec3::add(&blended, &Vec3::scalar_mul(w, &contrib));
                total_weight += w;
            }

            // Fall back to the first influence when all weights are zero
            // (e.g. weights stripped by the loader or a degenerate vertex);
            // keeps the vertex anchored to a bone rather than the origin.
            let final_v = if total_weight > 0.0 {
                if (total_weight - 1.0).abs() > 1e-4 {
                    Vec3::scalar_mul(1.0 / total_weight, &blended)
                } else {
                    blended
                }
            } else {
                let bone_id = ids[0];
                if bone_id < skin_mats.len() {
                    Vec3::crossed_mat(v, &skin_mats[bone_id])
                } else {
                    *v
                }
            };

            vertex_buffer.set_component(i, VertexComponents::POSITION, |p: &mut Vec3| {
                *p = final_v;
            });
        }
    }
}

ComObject_SkinnedMeshComponent!(super::SkinnedMeshComponent);

impl ISkinnedMeshComponentImpl for SkinnedMeshComponent {}

impl IComponentImpl for SkinnedMeshComponent {
    fn on_loading(&self) {
        self.load_geometries();
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, delta_sec: f32) {
        let rc = self.entity.get_rendering_component().unwrap();
        let objects = rc.render_objects();

        if objects.len() > 0 {
            let ro = objects[0].as_dyn();
            ro.update_vertices(&|vb: RefMut<VertexBuffer>| {
                self.update_vertex_buffer(vb);
            });
        }
    }
}

pub struct ArmatureComponent {
    // Held so the armature owns a refcount on the entity it's attached to;
    // dropping this field would change the entity's ownership graph.
    #[allow(dead_code)]
    entity: ComRc<IEntity>,
    root_bone: ComRc<IEntity>,
    bones: Vec<ComRc<IEntity>>,
    animation_state: RefCell<AnimationState>,
    animation_length: RefCell<f32>,
    animation_looping: RefCell<bool>,
    /// When set (and not looping), the clip plays once and then *holds*
    /// on its final keyframe instead of stopping and snapping back to
    /// the start. Used by PAL4 ACTION props with a finite `play-times`
    /// and `holding-end == 1`. Default `false` preserves the actor
    /// one-shot behaviour (stop + reset).
    animation_hold_end: RefCell<bool>,
    /// When set, the armature's `bones` are ordinary scene-graph *frame*
    /// entities (each carrying an [`HAnimBoneComponent`]) rather than a
    /// detached skinning skeleton. The engine already ticks those
    /// components and propagates their world transforms, so `on_updating`
    /// must NOT call `root_bone.update` / `update_world_transform`
    /// (that would double-advance the clock and clobber placement with
    /// an identity parent). The armature then only owns the shared
    /// timeline: loop-resetting every bone in phase, or holding on the
    /// final keyframe. Used by rigid, frame-hierarchy-animated PAL4
    /// props (doors, levers, tip markers). Default `false` is the
    /// skinned-actor path.
    frame_driven: bool,
    animation_tick: RefCell<f32>,
    event_manager: RefCell<AnimationEventManager>,
}

ComObject_ArmatureComponent!(super::ArmatureComponent);

impl ArmatureComponent {
    pub fn new(
        entity: ComRc<IEntity>,
        root_bone: ComRc<IEntity>,
        bones: Vec<ComRc<IEntity>>,
    ) -> Self {
        Self {
            entity,
            root_bone,
            bones,
            animation_state: RefCell::new(AnimationState::NoAnimation),
            animation_length: RefCell::new(0.),
            animation_looping: RefCell::new(false),
            animation_hold_end: RefCell::new(false),
            frame_driven: false,
            animation_tick: RefCell::new(0.),
            event_manager: RefCell::new(AnimationEventManager::new()),
        }
    }

    /// Construct a *frame-driven* armature: `bones` are scene-graph frame
    /// entities (each with an [`HAnimBoneComponent`]) animated in place by
    /// the engine. The armature only coordinates the shared timeline
    /// (loop reset / hold). `root_bone` is unused in this mode; the
    /// owning entity is passed for it purely to satisfy the field.
    /// See [`ArmatureComponent::frame_driven`].
    pub fn new_frame_driven(entity: ComRc<IEntity>, bones: Vec<ComRc<IEntity>>) -> Self {
        Self {
            entity: entity.clone(),
            root_bone: entity,
            bones,
            animation_state: RefCell::new(AnimationState::NoAnimation),
            animation_length: RefCell::new(0.),
            animation_looping: RefCell::new(false),
            animation_hold_end: RefCell::new(false),
            frame_driven: true,
            animation_tick: RefCell::new(0.),
            event_manager: RefCell::new(AnimationEventManager::new()),
        }
    }

    fn reset_animation_state(&self) {
        self.event_manager.borrow_mut().reset();
        self.animation_tick.replace(0.);

        // TODO: create a bone type to replace the whole IEntity stuff
        for b in &self.bones {
            b.get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap()
                .reset_timestamp();
        }
    }
}

impl IArmatureComponentImpl for ArmatureComponent {
    fn clear_animation(&self) {
        self.animation_state.replace(AnimationState::NoAnimation);
        for b in &self.bones {
            b.get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap()
                .set_keyframes(vec![]);
        }
    }

    fn set_looping(&self, looping: bool) {
        self.animation_looping.replace(looping);
    }

    fn add_animation_event_observer(&self, observer: ComRc<IAnimationEventObserver>) {
        self.event_manager.borrow_mut().add_observer(observer);
    }

    fn play(&self) {
        self.animation_state.replace(AnimationState::Playing);
    }

    fn pause(&self) {
        self.animation_state.replace(AnimationState::Paused);
    }

    fn stop(&self) {
        self.animation_state.replace(AnimationState::Stopped);
        self.reset_animation_state();
    }
}

impl ArmatureComponent {
    /// Inherent counterpart to the formerly-IDL `set_animation`.
    pub fn set_animation(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>) {
        let mut animation_length = 0.;
        for (bone, kf) in self.bones.iter().zip(keyframes) {
            let kf_animation_length = kf.last().unwrap().timestamp;
            if kf_animation_length > animation_length {
                animation_length = kf_animation_length;
            }

            bone.get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap()
                .set_keyframes(kf);
        }

        // Restart the clock for the new clip. Without this, every bone keeps
        // the `last_time` left over from the previous animation, so the first
        // rendered frame samples the freshly-installed keyframes at a stale
        // (often end-of-clip) time and the whole model snaps to a wrong pose
        // for one frame before the clock advances/loops back to the start.
        self.animation_tick.replace(0.);
        for b in &self.bones {
            b.get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap()
                .reset_timestamp();
        }

        self.animation_state.replace(AnimationState::Playing);
        self.animation_length.replace(animation_length);
        self.event_manager.borrow_mut().set_events(events);
    }

    /// Inherent setter for the hold-on-final-keyframe flag. See
    /// [`ArmatureComponent::animation_hold_end`]. Kept inherent (not on
    /// the COM interface) so it doesn't force an IDL regen; PAL4 object
    /// staging reaches it through [`IArmatureComponentExt`].
    pub fn set_hold_end(&self, hold_end: bool) {
        self.animation_hold_end.replace(hold_end);
    }

    /// Inherent counterpart to the formerly-IDL `animation_state`.
    pub fn animation_state(&self) -> AnimationState {
        *self.animation_state.borrow()
    }

    /// Debug accessor: current playback position.
    pub fn animation_tick(&self) -> f32 {
        *self.animation_tick.borrow()
    }

    /// Debug accessor: total length of the loaded animation in seconds.
    pub fn animation_length(&self) -> f32 {
        *self.animation_length.borrow()
    }

    /// Whether the armature is currently configured to restart the
    /// animation when its tick exceeds `animation_length` (the
    /// alternative is to `stop()`). The flag is set by
    /// [`Self::set_looping`] and consulted by [`Self::on_updating`].
    /// Exposed so script-side controllers can avoid waiting for a
    /// looping animation to "finish" — by definition it never will.
    pub fn animation_looping(&self) -> bool {
        *self.animation_looping.borrow()
    }

    /// Inherent counterpart to the formerly-IDL `bones`.
    pub fn bones(&self) -> Vec<ComRc<IEntity>> {
        self.bones.clone()
    }
}

/// Extension trait exposing `ArmatureComponent`'s formerly-IDL
/// accessors on a `ComRc<IArmatureComponent>` handle.
pub trait IArmatureComponentExt {
    fn set_animation(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>);
    fn set_hold_end(&self, hold_end: bool);
    fn animation_state(&self) -> AnimationState;
    fn bones(&self) -> Vec<ComRc<IEntity>>;
}

impl IArmatureComponentExt for ComRc<crate::comdef::IArmatureComponent> {
    fn set_animation(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>) {
        self.inner::<ArmatureComponent>()
            .set_animation(keyframes, events)
    }
    fn set_hold_end(&self, hold_end: bool) {
        self.inner::<ArmatureComponent>().set_hold_end(hold_end)
    }
    fn animation_state(&self) -> AnimationState {
        self.inner::<ArmatureComponent>().animation_state()
    }
    fn bones(&self) -> Vec<ComRc<IEntity>> {
        self.inner::<ArmatureComponent>().bones()
    }
}

impl IComponentImpl for ArmatureComponent {
    fn on_loading(&self) {}

    fn on_unloading(&self) {
        self.event_manager.borrow_mut().clear_observers();
    }

    fn on_updating(&self, delta_sec: f32) {
        if self.animation_state() == AnimationState::Playing {
            let new_tick = *self.animation_tick.borrow() + delta_sec;
            if new_tick > *self.animation_length.borrow() {
                if *self.animation_looping.borrow() {
                    self.reset_animation_state();
                } else if *self.animation_hold_end.borrow() || self.frame_driven {
                    // Freeze on the final keyframe: clamp the clock and
                    // pause. For the skinned path the trailing
                    // `root_bone.update` below still runs this frame
                    // (each bone clamps its own `last_time` to its final
                    // key), so the held pose is the end of the clip;
                    // `Paused` then keeps it there without resetting the
                    // timestamps to 0. For the frame-driven path the
                    // engine-ticked frame bones likewise clamp at their
                    // final key, so a non-looping clip naturally holds —
                    // we must never `stop()` (which resets to frame 0 and
                    // would let the self-ticking frame bones replay).
                    self.animation_tick.replace(*self.animation_length.borrow());
                    self.animation_state.replace(AnimationState::Paused);
                } else {
                    self.stop();
                }
            } else {
                self.animation_tick.replace(new_tick);
            }

            // Skinned skeletons live outside the scene graph, so the
            // armature must advance + world-recompute them itself. Frame
            // -driven props are ordinary scene-graph entities the engine
            // already ticks and propagates, so doing it here would
            // double-advance the clock and reset world placement to the
            // origin.
            if !self.frame_driven {
                self.root_bone.update(delta_sec);
                self.root_bone.update_world_transform(&Transform::new());
                self.event_manager.borrow_mut().tick(delta_sec);
            }
        }
    }
}

pub struct HAnimBoneComponent {
    entity: ComRc<IEntity>,
    // Stable bone id from the source asset, kept for diagnostics and future
    // lookup use even though no current path reads it.
    #[allow(dead_code)]
    id: u32,
    props: RefCell<HAnimBoneProps>,
}

ComObject_HAnimBoneComponent!(super::HAnimBoneComponent);

struct HAnimBoneProps {
    bond_pose: Mat44,
    frames: Vec<AnimKeyFrame>,
    last_time: f32,
    max_time: f32,
}

/// Select the keyframe segment containing `time` and the interpolation factor
/// within it.
///
/// Returns `(from_index, next_index, pct)` where `from_index` is the last
/// keyframe at or before `time`, `next_index` is the following keyframe (or
/// `from_index` itself at the end of the list), and `pct` is the clamped
/// `[0, 1]` blend factor across the segment's *duration*.
///
/// Extracted as a pure function so the off-by-one / denominator behaviour can
/// be unit-tested without an entity. `frames` is assumed non-empty and sorted
/// by ascending `timestamp` (the loaders guarantee both).
fn sample_frames(frames: &[AnimKeyFrame], time: f32) -> (usize, usize, f32) {
    let from_index = frames
        .iter()
        .rposition(|t| t.timestamp <= time)
        .unwrap_or(0);

    let next_index = (from_index + 1).min(frames.len() - 1);

    let from_ts = frames[from_index].timestamp;
    let next_ts = frames[next_index].timestamp;
    let segment = next_ts - from_ts;
    let pct = if from_index == next_index || segment <= 0.0 {
        0.
    } else {
        ((time - from_ts) / segment).clamp(0.0, 1.0)
    };

    (from_index, next_index, pct)
}

impl HAnimBoneProps {
    pub fn update(&mut self, entity: ComRc<IEntity>, delta_sec: f32) {
        self.last_time = self.last_time + delta_sec;

        // Hold on the final keyframe once this bone runs out of keys instead of
        // wrapping back to 0. Looping is the armature's responsibility: it
        // resets *every* bone's timestamp together via `reset_animation_state`,
        // which keeps all bones in phase. Wrapping here per-bone — using each
        // bone's own `max_time`, which differs because bones have different
        // keyframe counts/durations — desynced the skeleton and made the mesh
        // appear to split into disconnected parts.
        if self.last_time > self.max_time {
            self.last_time = self.max_time;
        }

        let (from_index, next_frame_index, pct) = sample_frames(&self.frames, self.last_time);

        let rotation = Quaternion::slerp(
            &self.frames[from_index].rotation,
            &self.frames[next_frame_index].rotation,
            pct,
        );

        let position = Vec3::lerp(
            &self.frames[from_index].position,
            &self.frames[next_frame_index].position,
            pct,
        );

        let mut frame_mat = rotation.to_rotate_matrix();
        frame_mat[0][3] = position.x;
        frame_mat[1][3] = position.y;
        frame_mat[2][3] = position.z;

        let b = entity.transform();
        b.borrow_mut().set_matrix(frame_mat);
    }
}

impl HAnimBoneComponent {
    pub fn new(entity: ComRc<IEntity>, id: u32) -> Self {
        Self {
            entity,
            id,
            props: RefCell::new(HAnimBoneProps {
                bond_pose: Mat44::new_identity(),
                frames: vec![],
                last_time: 0.,
                max_time: 0.,
            }),
        }
    }
}

impl IHAnimBoneComponentImpl for HAnimBoneComponent {
    fn reset_timestamp(&self) {
        self.props.borrow_mut().last_time = 0.;
    }
}

impl HAnimBoneComponent {
    /// Inherent counterpart to the formerly-IDL `set_keyframes`.
    pub fn set_keyframes(&self, keyframes: Vec<AnimKeyFrame>) {
        self.props.borrow_mut().max_time = keyframes.last().unwrap().timestamp;
        self.props.borrow_mut().frames = keyframes;
    }

    /// Inherent counterpart to the formerly-IDL `set_bond_pose`.
    pub fn set_bond_pose(&self, matrix: Mat44) {
        self.props.borrow_mut().bond_pose = matrix;
    }

    /// Inherent counterpart to the formerly-IDL `bond_pose`.
    pub fn bond_pose(&self) -> Mat44 {
        self.props.borrow().bond_pose.clone()
    }
}

/// Extension trait exposing `HAnimBoneComponent`'s formerly-IDL
/// accessors on a `ComRc<IHAnimBoneComponent>` handle.
pub trait IHAnimBoneComponentExt {
    fn set_keyframes(&self, keyframes: Vec<AnimKeyFrame>);
    fn set_bond_pose(&self, matrix: Mat44);
    fn bond_pose(&self) -> Mat44;
}

impl IHAnimBoneComponentExt for ComRc<crate::comdef::IHAnimBoneComponent> {
    fn set_keyframes(&self, keyframes: Vec<AnimKeyFrame>) {
        self.inner::<HAnimBoneComponent>().set_keyframes(keyframes)
    }
    fn set_bond_pose(&self, matrix: Mat44) {
        self.inner::<HAnimBoneComponent>().set_bond_pose(matrix)
    }
    fn bond_pose(&self) -> Mat44 {
        self.inner::<HAnimBoneComponent>().bond_pose()
    }
}

impl IComponentImpl for HAnimBoneComponent {
    fn on_loading(&self) {}

    fn on_unloading(&self) {}

    fn on_updating(&self, delta_sec: f32) {
        if self.props.borrow().frames.is_empty() {
            return;
        }

        self.props
            .borrow_mut()
            .update(self.entity.clone(), delta_sec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Quaternion, Vec3};

    fn kf(timestamp: f32) -> AnimKeyFrame {
        AnimKeyFrame {
            rotation: Quaternion::new(0., 0., 0., 1.),
            position: Vec3::new(0., 0., 0.),
            timestamp,
        }
    }

    #[test]
    fn sample_picks_segment_at_or_before_time() {
        let frames = vec![kf(0.), kf(0.1), kf(0.2), kf(0.3)];
        // 0.15 sits in the [0.1, 0.2] segment, half-way through.
        let (from, next, pct) = sample_frames(&frames, 0.15);
        assert_eq!((from, next), (1, 2));
        assert!((pct - 0.5).abs() < 1e-6, "pct = {pct}");
    }

    #[test]
    fn sample_pct_uses_segment_duration_not_abs_timestamp() {
        // Non-uniform spacing: a 0.4-wide segment between 0.6 and 1.0.
        let frames = vec![kf(0.), kf(0.6), kf(1.0)];
        let (from, next, pct) = sample_frames(&frames, 0.8);
        assert_eq!((from, next), (1, 2));
        // (0.8 - 0.6) / (1.0 - 0.6) = 0.5, not 0.8 / 1.0.
        assert!((pct - 0.5).abs() < 1e-6, "pct = {pct}");
    }

    #[test]
    fn sample_clamps_at_and_past_end() {
        let frames = vec![kf(0.), kf(0.1), kf(0.2)];
        let (from, next, pct) = sample_frames(&frames, 0.2);
        assert_eq!((from, next), (2, 2));
        assert_eq!(pct, 0.0);

        // Past the end (would only happen transiently) still clamps, never
        // extrapolates.
        let (from, next, pct) = sample_frames(&frames, 5.0);
        assert_eq!((from, next), (2, 2));
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn sample_before_first_keyframe_holds_first() {
        let frames = vec![kf(0.5), kf(1.0)];
        let (from, next, pct) = sample_frames(&frames, 0.0);
        assert_eq!((from, next), (0, 1));
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn sample_single_keyframe_holds() {
        let frames = vec![kf(0.)];
        let (from, next, pct) = sample_frames(&frames, 1.0);
        assert_eq!((from, next), (0, 0));
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn bones_of_different_length_stay_in_phase() {
        // Two bones, different keyframe durations. At the same shared clock
        // time they must agree on phase: the longer bone keeps interpolating
        // while the shorter one holds its final pose, rather than wrapping and
        // desyncing (the "broken into parts" bug).
        let long = vec![kf(0.), kf(0.2), kf(0.4)];
        let short = vec![kf(0.), kf(0.2)];

        let t = 0.3;
        let (lf, ln, lpct) = sample_frames(&long, t);
        assert_eq!((lf, ln), (1, 2));
        assert!((lpct - 0.5).abs() < 1e-6);

        // Shorter bone is past its last keyframe: holds, never wraps to 0.
        let (sf, sn, spct) = sample_frames(&short, t);
        assert_eq!((sf, sn), (1, 1));
        assert_eq!(spct, 0.0);
    }
}
