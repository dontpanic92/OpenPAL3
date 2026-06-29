//! PAL3 character blob shadow.
//!
//! The original game (`roleshadow.cpp` / `shadowquad.gbf` in basedata.cpk)
//! draws a fixed, soft-edged circle (`shadow.tga`) as a flat ground quad
//! under every role rather than computing real lighting. We reproduce that
//! faithful blob: a horizontal, double-sided, alpha-blended quad parented to
//! the role entity at its feet (local y ≈ 0). Because it is a child entity it
//! inherits the role's world transform but does not deform with the MV3
//! animation, and the scene renderer only collects children of a visible
//! parent, so the blob hides automatically when the role is hidden.

use std::path::Path;
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::{IEntity, IStaticMeshComponent};
use radiance::components::mesh::{Geometry, StaticMeshComponent, TexCoord};
use radiance::math::Vec3;
use radiance::rendering::{BlendMode, ComponentFactory, SimpleMaterialDef};
use radiance::scene::CoreEntity;

/// Half-width of the shadow quad, in role local space. The blob spans
/// `2 * RADIUS` on each axis; tuned to roughly cover a character footprint.
const RADIUS: f32 = 30.0;

/// Small upward bias so the disc renders just above the ground plane and
/// doesn't z-fight with the terrain it sits on.
const Y_BIAS: f32 = 1.0;

/// Build a flat circle blob shadow child entity from `shadow.tga`. Returns
/// `None` if the texture can't be loaded, so roles still render shadowless.
pub fn build_role_shadow(
    factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    basedata_path: &Path,
) -> Option<ComRc<IEntity>> {
    let image = load_texture(vfs, &basedata_path.join("shadow.tga"))?;

    let verts = vec![
        Vec3::new(-RADIUS, Y_BIAS, -RADIUS),
        Vec3::new(RADIUS, Y_BIAS, -RADIUS),
        Vec3::new(RADIUS, Y_BIAS, RADIUS),
        Vec3::new(-RADIUS, Y_BIAS, RADIUS),
    ];
    let texcoords: Vec<TexCoord> = vec![
        TexCoord::new(0.0, 0.0),
        TexCoord::new(1.0, 0.0),
        TexCoord::new(1.0, 1.0),
        TexCoord::new(0.0, 1.0),
    ];
    // Double-sided so the disc shows regardless of cull winding / facing.
    let indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2];

    let material = SimpleMaterialDef::create_with_image("role_shadow", Some(image))
        .with_blend(BlendMode::Multiply)
        .make_unique();
    let geometry = Geometry::new(
        &verts,
        None,
        std::slice::from_ref(&texcoords),
        indices,
        material,
    );

    let entity = CoreEntity::create("ROLE_SHADOW".to_string(), true);
    let mesh = StaticMeshComponent::new(entity.clone(), vec![geometry], factory.clone());
    entity.add_component(IStaticMeshComponent::uuid(), ComRc::from_object(mesh));
    Some(entity)
}

/// Decode a PAL3 `.tga` texture upright (PAL3 textures are bottom-up D3D9).
fn load_texture(vfs: &MiniFs, path: &Path) -> Option<image::RgbaImage> {
    use std::io::Read;
    let mut file = vfs.open(path).ok()?;
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).ok()?;
    let img = image::load_from_memory(&bytes)
        .or_else(|_| image::load_from_memory_with_format(&bytes, image::ImageFormat::Tga))
        .ok()?;
    Some(img.flipv().to_rgba8())
}
