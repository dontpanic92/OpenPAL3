use crosscom::ComRc;
use fileformats::pol::{PolMaterialInfo, PolTriangle, PolVertex, read_pol};
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::{IEntity, IStaticMeshComponent};
use radiance::components::mesh::{Geometry, StaticMeshComponent, TexCoord};
use radiance::math::Vec3;
use radiance::rendering::{
    BlendMode, ComponentFactory, LightMapMaterialDef, LitMaterialDef, MaterialDef,
};
use radiance::scene::CoreEntity;
use std::io::BufReader;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

pub fn create_entity_from_pol_model<P: AsRef<Path>>(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
    name: String,
    visible: bool,
) -> ComRc<IEntity> {
    let entity = CoreEntity::create(name, visible);
    let geometries = load_pol_model(vfs, path);
    let mesh_component =
        StaticMeshComponent::new(entity.clone(), geometries, component_factory.clone());
    entity.add_component(
        IStaticMeshComponent::uuid(),
        crosscom::ComRc::from_object(mesh_component),
    );
    entity
}

fn load_pol_model<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> Vec<Geometry> {
    let mut reader = BufReader::new(vfs.open(&path).unwrap());
    let pol = read_pol(&mut reader).unwrap();
    let mut geometries = vec![];
    for mesh in &pol.meshes {
        for material in &mesh.material_info {
            // Single-texture POL surfaces carry no baked lightmap, so — like
            // the original engine — they are shaded by the scene's dynamic
            // `.lgt` lights (see `load_material`). Those need per-vertex
            // normals; multi-texture (lightmapped) surfaces keep the baked
            // path and omit normals so the lightmap pipeline's vertex layout
            // is unchanged.
            let lit = material.texture_count == 1;
            let geometry = create_geometry(
                &mesh.vertices,
                &material.triangles,
                load_material(&material, vfs, path.as_ref()),
                lit,
            );

            geometries.push(geometry);
        }
    }

    geometries
}

fn load_material<P: AsRef<Path>>(material: &PolMaterialInfo, vfs: &MiniFs, path: P) -> MaterialDef {
    let texture_paths: Vec<PathBuf> = material
        .texture_names
        .iter()
        .map(|name| {
            let name = &name.as_str().unwrap();
            name.split_terminator('.')
                .next()
                .and_then(|n| Some(n.to_owned() + ".dds"))
                .and_then(|dds_name| {
                    let mut texture_path = path.as_ref().to_owned();
                    texture_path.pop();
                    texture_path.push(dds_name);
                    if !vfs.open(&texture_path).is_ok() {
                        texture_path.pop();
                        texture_path.push(name);
                    }

                    Some(texture_path)
                })
                .or(Some(PathBuf::from(name)))
                .unwrap()
        })
        .collect();

    // PAL3 `.pol` materials carry `use_alpha`: 0 means the surface is
    // fully opaque (no cutout, no blending), non-zero means the legacy
    // alpha-test path (cutout against `MaterialParams::alpha_ref`).
    let blend = if material.use_alpha == 0 {
        BlendMode::Opaque
    } else {
        BlendMode::AlphaTest
    };

    if texture_paths.len() == 1 {
        // No baked lightmap: shade dynamically from the scene `.lgt` lights
        // (matches the original engine, which lit these placed props/surfaces
        // with the D3D scene lights). `create_geometry` supplies normals.
        LitMaterialDef::create(texture_paths[0].to_str().unwrap(), |name| vfs.open(name).ok())
            .with_blend(blend)
    } else {
        let textures: Vec<_> = texture_paths.iter().map(|p| p.to_str().unwrap()).collect();
        LightMapMaterialDef::create(textures, |name| {
            PathBuf::from(name)
                .file_stem()
                .and_then(|_| vfs.open(name).ok())
        })
        .with_blend(blend)
        // PAL3's baked lightmaps are the primary scene lighting. Apply them
        // faithfully: gain ≈ 2.0 (intensity 1.333 × the shader's 1.5) with no
        // ambient floor, so shadowed areas stay dark and the warm baked tone
        // isn't washed toward grey. (The shared shader's 0.3 floor was tuned
        // for PAL4's dark caves; PAL3 opts out via ambient_floor = 0.0.)
        .with_lightmap_params(1.333, 0.0)
    }
}

fn create_geometry(
    all_vertices: &Vec<PolVertex>,
    triangles: &[PolTriangle],
    material: MaterialDef,
    lit: bool,
) -> Geometry {
    let mut index_map = std::collections::HashMap::new();
    let mut reversed_index = vec![];
    let mut get_new_index = |index: u16| -> u32 {
        if index_map.contains_key(&index) {
            index_map[&index]
        } else {
            let new_index = reversed_index.len() as u32;
            reversed_index.push(index as usize);
            index_map.insert(index, new_index);
            new_index
        }
    };

    let mut indices: Vec<u32> = vec![];
    for t in triangles {
        indices.push(get_new_index(t.indices[0]));
        indices.push(get_new_index(t.indices[1]));
        indices.push(get_new_index(t.indices[2]));
    }

    let mut vertices = vec![];

    let mut texcoord1 = vec![];
    let mut texcoord2 = vec![];

    for i in 0..reversed_index.len() {
        let vert = &all_vertices[reversed_index[i]];
        let v = Vec3::new(vert.position.x, vert.position.y, vert.position.z);
        vertices.push(v);
        texcoord1.push(TexCoord::new(vert.tex_coord.u, vert.tex_coord.v));

        if let Some(texcoord) = &vert.tex_coord2 {
            texcoord2.push(TexCoord::new(texcoord.u, texcoord.v));
        }
    }

    let texcoords = if texcoord2.is_empty() {
        vec![texcoord1]
    } else {
        // PAL3 `.pol` 2-texture materials are stored as
        // `texture_names = [lightmap, diffuse]` (verified by the
        // `^L_*.bmp` lightmap-name prefix in scene archives), with
        // `vert.tex_coord` carrying the **lightmap** UV and
        // `vert.tex_coord2` carrying the **diffuse** UV. The
        // `lightmap_texture.frag` shader (since commit f2c083a
        // "Support bsp lightmap") expects the opposite convention —
        // primary UV (`fragTexCoord`) → diffuse, secondary UV
        // (`fragTexCoord2`) → lightmap atlas. We swap the channels
        // here so PAL3 conforms to the shared shader without forcing
        // a PAL3-specific shader variant. Single-texture materials
        // (no `tex_coord2`) are unaffected.
        vec![texcoord2, texcoord1]
    };

    // Dynamically-lit surfaces need per-vertex normals. Prefer the `.pol`'s
    // stored normals; fall back to smooth geometric normals when the mesh
    // omits them. Lightmapped surfaces stay normal-less (baked lighting).
    let normals = if lit {
        Some(build_normals(all_vertices, &reversed_index, &vertices, &indices))
    } else {
        None
    };

    Geometry::new(
        &vertices,
        normals.as_deref(),
        &texcoords,
        indices,
        material,
    )
}

/// Build per-vertex normals (in the deduplicated `vertices` order) for a lit
/// POL surface. Uses the `.pol`'s stored vertex normals when present; otherwise
/// accumulates smooth geometric normals from the face winding.
fn build_normals(
    all_vertices: &[PolVertex],
    reversed_index: &[usize],
    vertices: &[Vec3],
    indices: &[u32],
) -> Vec<Vec3> {
    let has_stored = all_vertices
        .get(reversed_index.first().copied().unwrap_or(0))
        .map_or(false, |v| v.normal.is_some());

    if has_stored {
        return reversed_index
            .iter()
            .map(|&src| {
                all_vertices[src]
                    .normal
                    .as_ref()
                    .map(|n| normalize(Vec3::new(n.x, n.y, n.z)))
                    .unwrap_or_else(|| Vec3::new(0.0, 1.0, 0.0))
            })
            .collect();
    }

    let mut normals = vec![Vec3::new(0.0, 0.0, 0.0); vertices.len()];
    for tri in indices.chunks_exact(3) {
        let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let e1 = sub(vertices[b], vertices[a]);
        let e2 = sub(vertices[c], vertices[a]);
        let fn_ = cross(e1, e2);
        for &idx in &[a, b, c] {
            normals[idx] = add(normals[idx], fn_);
        }
    }
    for n in normals.iter_mut() {
        *n = normalize(*n);
        if n.x == 0.0 && n.y == 0.0 && n.z == 0.0 {
            *n = Vec3::new(0.0, 1.0, 0.0);
        }
    }
    normals
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn normalize(v: Vec3) -> Vec3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len > 1e-6 {
        Vec3::new(v.x / len, v.y / len, v.z / len)
    } else {
        Vec3::new(0.0, 0.0, 0.0)
    }
}
