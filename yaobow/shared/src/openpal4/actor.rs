use crosscom::ComRc;
use radiance::{
    comdef::{IArmatureComponent, IEntity},
    components::mesh::skinned_mesh::AnimKeyFrame,
};

pub struct Pal4Actor;

impl Pal4Actor {
    pub fn set_anim(entity: ComRc<IEntity>, anm: &Vec<Vec<AnimKeyFrame>>) {
        for e in &entity.children() {
            let x = || -> Option<()> {
                e.get_component(IArmatureComponent::uuid())?
                    .query_interface::<IArmatureComponent>()?
                    .set_keyframes(anm.clone());
                Some(())
            };

            let _ = x();
        }
    }
}
