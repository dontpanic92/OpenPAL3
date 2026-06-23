use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crosscom::ComRc;

use crate::{
    comdef::{IAnimatedMeshComponentImpl, IComponentImpl, IEntity, IEntityExt},
    math::{Vec2, Vec3},
    rendering::{ComponentFactory, VertexBuffer, VertexComponents},
};

use super::{Geometry, TexCoord, morph_target::MorphTarget};

pub struct AnimatedMeshComponent {
    entity: ComRc<IEntity>,
    component_factory: Rc<dyn ComponentFactory>,
    props: RefCell<AnimatedMeshComponentProps>,
}

#[derive(PartialEq, Copy, Clone)]
pub enum MorphAnimationState {
    NotStarted,
    Playing,
    Holding,
    Finished,
}

struct AnimatedMeshComponentProps {
    morph_targets: Vec<MorphTarget>,
    morph_animation_state: MorphAnimationState,
    last_time: f32,
    hold_time: f32,
    hold: bool,
    hold_enabled: bool,
    loop_playback: bool,
}

ComObject_AnimatedMeshComponent!(super::AnimatedMeshComponent);

impl AnimatedMeshComponent {
    pub fn new(entity: ComRc<IEntity>, component_factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            entity,
            component_factory,
            props: RefCell::new(AnimatedMeshComponentProps {
                morph_targets: vec![],
                morph_animation_state: MorphAnimationState::NotStarted,
                last_time: 0.,
                hold_time: 0.,
                hold: false,
                hold_enabled: true,
                loop_playback: false,
            }),
        }
    }

    fn props_mut(&self) -> RefMut<'_, AnimatedMeshComponentProps> {
        self.props.borrow_mut()
    }

    fn props(&self) -> Ref<'_, AnimatedMeshComponentProps> {
        self.props.borrow()
    }

    pub fn morph_animation_length(&self) -> Option<f32> {
        self.props()
            .morph_targets
            .last()
            .and_then(|m| Some(m.timestamp))
    }

    pub fn add_morph_last_time(&self, time: f32) {
        self.props_mut().last_time += time;
    }

    pub fn reset_morph_last_time(&self) {
        self.props_mut().last_time = 0.;
    }

    pub fn set_morph_hold(&self) {
        self.props_mut().hold = true;
    }

    pub fn reset_morph_hold(&self) {
        self.props_mut().hold = false;
    }

    /// Arm or disarm the mid-animation "hold" pause. The MV3 "hold" action
    /// marker is an SCE-controlled pause point: the animation freezes there
    /// until `SceCommandRoleEndAction` resumes it (see
    /// `openpal3::scene::role_controller`). Only the SCE `RoleShowAction`
    /// "hold" mode (`-2`) should honour it — looping/repeating actions (idle,
    /// walk, run) must play straight through, otherwise they visibly stutter
    /// at the marker every cycle. Defaults to enabled for backwards
    /// compatibility with callers that don't set a play mode.
    pub fn set_hold_enabled(&self, enabled: bool) {
        self.props_mut().hold_enabled = enabled;
    }

    /// Mark playback as looping. A looping animation *wraps* at its end rather
    /// than *ending* on its final pose, so the exact last keyframe must NOT be
    /// presented as a held frame: many MV3 loop clips (e.g. several characters'
    /// run cycles) have a final keyframe that is a distinct pose from frame 0
    /// (a non-closed loop seam). Presenting it for one frame before wrapping to
    /// frame 0 makes the actor visibly flash a wrong pose every cycle. When
    /// looping, `on_updating` wraps without that final-frame render. Non-loop
    /// playback (one-shot / `RoleShowAction` hold `-2`) keeps presenting the
    /// final frame so an action settles on its true end pose. Defaults to off.
    pub fn set_loop_playback(&self, looping: bool) {
        self.props_mut().loop_playback = looping;
    }

    pub fn is_to_hold(&self) -> bool {
        self.props().hold_enabled
            && !self.props().hold
            && self.props().hold_time > 0.
            && self.props().last_time > self.props().hold_time
    }

    pub fn is_to_end(&self) -> bool {
        self.props().last_time > self.morph_animation_length().unwrap()
    }

    pub fn set_morph_targets(&self, morph_targets: Vec<MorphTarget>, hold_time: f32) {
        self.props_mut().morph_targets = morph_targets;
        self.props_mut().morph_animation_state = MorphAnimationState::Playing;
        self.props_mut().hold_time = hold_time;
        self.reset_morph_last_time();

        if self.props().morph_targets.is_empty() {
            return;
        }

        self.load_geometries(&self.props().morph_targets[0].geometries);
    }

    /// Resolve the keyframe pair and blend factor for `anim_timestamp`.
    ///
    /// `anim_timestamp` is clamped to `[0, animation_length]` so the final
    /// keyframe can be addressed at full weight. When the timestamp lands on
    /// or past the last keyframe, this returns `(last, last, 0.0)`, i.e. the
    /// last frame rendered exactly (no underflow, no wrap-around
    /// extrapolation). Returns `None` only when there are no morph targets.
    fn resolve_frame(
        morph_targets: &[MorphTarget],
        anim_timestamp: f32,
    ) -> Option<(usize, usize, f32)> {
        resolve_frame_at(morph_targets.iter().map(|t| t.timestamp), anim_timestamp)
    }

    pub fn update_morph_target(
        &self,
        anim_timestamp: f32,
        g_index: usize,
        mut vertex_buffer: RefMut<VertexBuffer>,
    ) {
        let props = self.props();
        let (frame_index, next_frame_index, percentile) =
            match Self::resolve_frame(&props.morph_targets, anim_timestamp) {
                Some(v) => v,
                None => return,
            };

        let target = &props.morph_targets.get(frame_index).unwrap().geometries;
        let next_target = &props
            .morph_targets
            .get(next_frame_index)
            .unwrap()
            .geometries;

        // for (t, nt) in target.iter().zip(next_target) {
        let t = target.get(g_index);
        let nt = next_target.get(g_index);
        if t.is_none() || nt.is_none() {
            return;
        }

        let t = t.unwrap();
        let nt = nt.unwrap();

        for i in 0..vertex_buffer.count() {
            let position = t.vertices.position(i).unwrap();
            let next_position = nt.vertices.position(i).unwrap();
            let tex_coord = t.vertices.tex_coord(i);
            let normal = t.vertices.normal(i).cloned();
            let next_normal = nt.vertices.normal(i).cloned();

            vertex_buffer.set_component(i, VertexComponents::POSITION, |p: &mut Vec3| {
                p.x = position.x * (1. - percentile) + next_position.x * percentile;
                p.y = position.y * (1. - percentile) + next_position.y * percentile;
                p.z = position.z * (1. - percentile) + next_position.z * percentile;
            });

            // Morph the per-vertex normal alongside the position so dynamically
            // lit actors (`actor_lit`) re-shade as the mesh deforms. Without
            // this the normals stay frozen at the loaded pose and the lit side
            // of the actor never follows the animation. The interpolated normal
            // need not be unit-length here — `actor_lit.vert` normalizes it.
            if let (Some(n), Some(nn)) = (normal, next_normal) {
                vertex_buffer.set_component(i, VertexComponents::NORMAL, |out: &mut Vec3| {
                    out.x = n.x * (1. - percentile) + nn.x * percentile;
                    out.y = n.y * (1. - percentile) + nn.y * percentile;
                    out.z = n.z * (1. - percentile) + nn.z * percentile;
                });
            }

            if let Some(tex_coord) = tex_coord {
                vertex_buffer.set_component(i, VertexComponents::TEXCOORD, |t: &mut Vec2| {
                    t.x = tex_coord.x;
                    t.y = tex_coord.y;
                });
            }
        }
        // }
    }

    pub fn blend_morph_target(&self, anim_timestamp: f32) -> Vec<Geometry> {
        let props = self.props();
        let (frame_index, next_frame_index, percentile) =
            match Self::resolve_frame(&props.morph_targets, anim_timestamp) {
                Some(v) => v,
                None => return vec![],
            };

        let target = &props.morph_targets.get(frame_index).unwrap().geometries;
        let next_target = &props
            .morph_targets
            .get(next_frame_index)
            .unwrap()
            .geometries;

        let mut blended = vec![];
        for (t, nt) in target.iter().zip(next_target) {
            let mut vertices = vec![];
            let mut normals = vec![];
            let mut texcoord_vec = vec![];
            let mut texcoord2_vec = vec![];

            for i in 0..t.vertices.count() {
                let position = t.vertices.position(i).unwrap();
                let next_position = nt.vertices.position(i).unwrap();
                vertices.push(Self::blend_vec3(position, next_position, percentile));

                let normal = t.vertices.normal(i);
                if let Some(normal) = normal {
                    let next_normal = nt.vertices.normal(i).unwrap();
                    normals.push(Self::blend_vec3(normal, next_normal, percentile));
                }

                let tex_coord = t.vertices.tex_coord(i);
                if let Some(tex_coord) = tex_coord {
                    texcoord_vec.push(TexCoord::new(tex_coord.x, tex_coord.y));
                }

                let tex_coord2 = t.vertices.tex_coord2(i);
                if let Some(tex_coord) = tex_coord2 {
                    texcoord2_vec.push(TexCoord::new(tex_coord.x, tex_coord.y));
                }
            }

            let mut texcoords = vec![];
            if texcoord_vec.len() > 0 {
                texcoords.push(texcoord_vec);
            }

            if texcoord2_vec.len() > 0 {
                texcoords.push(texcoord2_vec);
            }

            let normals = if normals.len() > 0 {
                Some(normals.as_ref())
            } else {
                None
            };

            let geometry = Geometry::new(
                &vertices,
                normals,
                &texcoords,
                t.indices.clone(),
                t.material.clone(),
            );
            blended.push(geometry);
        }

        blended
    }

    fn blend_vec3(v1: &Vec3, v2: &Vec3, v2_p: f32) -> Vec3 {
        Vec3::new(
            v1.x * (1. - v2_p) + v2.x * v2_p,
            v1.y * (1. - v2_p) + v2.y * v2_p,
            v1.z * (1. - v2_p) + v2.z * v2_p,
        )
    }

    fn _blend_vec2(v1: &Vec2, v2: &Vec2, v2_p: f32) -> Vec2 {
        Vec2::new(
            v1.x * (1. - v2_p) + v2.x * v2_p,
            v1.y * (1. - v2_p) + v2.y * v2_p,
        )
    }

    fn load_geometries(&self, geometries: &[Geometry]) {
        let mut objects = vec![];
        for geometry in geometries {
            let ro = self.component_factory.create_render_object(
                geometry.vertices.clone(),
                geometry.indices.clone(),
                &geometry.material,
                true,
            );

            objects.push(ro);
        }

        let component = self.component_factory.create_rendering_component(objects);
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn render(&self, timestamp: f32) {
        if self.entity.get_rendering_component().is_none() {
            return;
        }

        let rc = self.entity.get_rendering_component().unwrap();
        let objects = rc.render_objects();

        for i in 0..objects.len() {
            let ro = objects[i].as_dyn();
            ro.update_vertices(&|vb: RefMut<VertexBuffer>| {
                self.update_morph_target(timestamp, i, vb);
            });
        }
    }
}

impl IComponentImpl for AnimatedMeshComponent {
    fn on_loading(&self) -> crosscom::Void {
        self.load_geometries(&self.props().morph_targets[0].geometries);
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {
        let last_time = self.props().last_time;
        let anim_state = self.props().morph_animation_state;

        if self.props().morph_targets.is_empty() {
            return;
        }

        if anim_state == MorphAnimationState::Playing {
            if self.is_to_end() {
                let length = self.morph_animation_length().unwrap();
                if self.props().loop_playback {
                    // Looping playback (idle/walk/run) wraps *continuously*
                    // instead of ending: carry the phase overshoot past the
                    // loop point and keep playing. This avoids the
                    // `Finished -> replay` round-trip, which (a) restarted the
                    // clip at frame 0 with a one-frame hold/snap at the seam and
                    // (b) flipped `RoleState` to `AnimationFinished`, making the
                    // free-roam mover re-trigger `run()` every cycle and reset
                    // the run animation — the visible per-loop hitch. Staying in
                    // `Playing` keeps `RoleState` stable so no re-trigger fires.
                    let wrapped = (last_time - length).max(0.0);
                    self.props_mut().last_time = wrapped;
                    self.reset_morph_hold();
                    self.render(wrapped);
                    return;
                }

                // Non-looping clip (one-shot / hold): present the exact final
                // keyframe before finishing so the action settles on its true
                // end pose. Without this it stops one tick short (the pose is
                // only ever approached as `percentile -> 1`). `resolve_frame`
                // clamps to the animation length, so rendering at it is safe.
                self.render(length);
                self.props_mut().morph_animation_state = MorphAnimationState::Finished;
                self.reset_morph_last_time();
                self.reset_morph_hold();
                return;
            } else if self.is_to_hold() {
                self.props_mut().morph_animation_state = MorphAnimationState::Holding;
                self.set_morph_hold();
            } else {
                self.add_morph_last_time(delta_sec);
            }

            self.render(last_time);
        } else if anim_state == MorphAnimationState::Holding {
            self.render(last_time);
        }
    }
}

impl IAnimatedMeshComponentImpl for AnimatedMeshComponent {
    fn play(&self, replay: bool) -> () {
        if replay {
            self.reset_morph_last_time();
            self.reset_morph_hold();
        }
        self.props_mut().morph_animation_state = MorphAnimationState::Playing;
    }
}

impl AnimatedMeshComponent {
    /// Inherent counterpart to the formerly-IDL `morph_animation_state`.
    /// Access from a `ComRc<IAnimatedMeshComponent>` via
    /// [`IAnimatedMeshComponentExt`].
    pub fn morph_animation_state(&self) -> MorphAnimationState {
        self.props().morph_animation_state
    }
}

pub trait IAnimatedMeshComponentExt {
    fn morph_animation_state(&self) -> MorphAnimationState;
    fn set_hold_enabled(&self, enabled: bool);
    fn set_loop_playback(&self, looping: bool);
}

impl IAnimatedMeshComponentExt for ComRc<crate::comdef::IAnimatedMeshComponent> {
    fn morph_animation_state(&self) -> MorphAnimationState {
        self.inner::<AnimatedMeshComponent>()
            .morph_animation_state()
    }

    fn set_hold_enabled(&self, enabled: bool) {
        self.inner::<AnimatedMeshComponent>()
            .set_hold_enabled(enabled);
    }

    fn set_loop_playback(&self, looping: bool) {
        self.inner::<AnimatedMeshComponent>()
            .set_loop_playback(looping);
    }
}

/// Pure keyframe-resolution math shared by `update_morph_target` and
/// `blend_morph_target`. Takes the keyframe timestamps (assumed ascending,
/// first == 0) and a play head, and returns `(frame_index, next_frame_index,
/// percentile)`. The play head is clamped to `[0, last_timestamp]`; at/after
/// the last keyframe it returns `(last, last, 0.0)` so the final pose renders
/// exactly instead of being skipped or extrapolated.
fn resolve_frame_at(
    timestamps: impl IntoIterator<Item = f32>,
    anim_timestamp: f32,
) -> Option<(usize, usize, f32)> {
    let timestamps: Vec<f32> = timestamps.into_iter().collect();
    if timestamps.is_empty() {
        return None;
    }

    let last_index = timestamps.len() - 1;
    let length = timestamps[last_index];
    let anim_timestamp = anim_timestamp.clamp(0., length);

    let frame_index = timestamps
        .iter()
        .position(|&t| t > anim_timestamp)
        .map(|i| i.saturating_sub(1))
        .unwrap_or(last_index);

    let next_frame_index = (frame_index + 1) % timestamps.len();
    let span = timestamps[next_frame_index] - timestamps[frame_index];
    let percentile = if span > 0. {
        (anim_timestamp - timestamps[frame_index]) / span
    } else {
        // Final keyframe (next wrapped to 0) or coincident timestamps: hold
        // the current frame exactly instead of extrapolating.
        0.
    };

    Some((frame_index, next_frame_index, percentile))
}

#[cfg(test)]
mod tests {
    use super::resolve_frame_at;

    const TS: [f32; 4] = [0.0, 0.1, 0.2, 0.3];

    #[test]
    fn empty_returns_none() {
        assert!(resolve_frame_at(std::iter::empty::<f32>(), 0.0).is_none());
    }

    #[test]
    fn start_uses_first_segment() {
        let (f, n, p) = resolve_frame_at(TS, 0.0).unwrap();
        assert_eq!((f, n), (0, 1));
        assert!((p - 0.0).abs() < 1e-6);
    }

    #[test]
    fn midpoint_interpolates() {
        let (f, n, p) = resolve_frame_at(TS, 0.15).unwrap();
        assert_eq!((f, n), (1, 2));
        assert!((p - 0.5).abs() < 1e-6);
    }

    #[test]
    fn last_segment_before_end() {
        let (f, n, p) = resolve_frame_at(TS, 0.29).unwrap();
        assert_eq!((f, n), (2, 3));
        assert!(p > 0.85 && p < 1.0);
    }

    // The regression this fix targets: at the final timestamp the last
    // keyframe must be addressable at full weight (percentile 0 on the last
    // frame), not skipped or extrapolated past.
    #[test]
    fn exact_end_shows_last_frame() {
        let (f, n, p) = resolve_frame_at(TS, 0.3).unwrap();
        assert_eq!(f, 3, "play head at length must resolve to the last frame");
        assert_eq!(n, 0, "next wraps but percentile pins to the last frame");
        assert!((p - 0.0).abs() < 1e-6);
    }

    #[test]
    fn past_end_is_clamped_to_last_frame() {
        let (f, _n, p) = resolve_frame_at(TS, 99.0).unwrap();
        assert_eq!(f, 3);
        assert!((p - 0.0).abs() < 1e-6);
    }

    #[test]
    fn negative_clamps_to_start() {
        let (f, n, p) = resolve_frame_at(TS, -5.0).unwrap();
        assert_eq!((f, n), (0, 1));
        assert!((p - 0.0).abs() < 1e-6);
    }

    #[test]
    fn single_frame_holds() {
        let (f, n, p) = resolve_frame_at([0.0], 0.0).unwrap();
        assert_eq!((f, n), (0, 0));
        assert!((p - 0.0).abs() < 1e-6);
    }
}
