use obj_exporter::{Geometry, ObjSet, Object, Shape, TVertex, Vertex};
use opengb::loaders::pol_loader::{PolFile, PolVertexComponents};

pub fn export_pol_to_obj(pol_file: Option<&PolFile>) -> Option<ObjSet> {
    if pol_file.is_none() {
        return None;
    }

    let pol_file = pol_file.unwrap();
    let objects = pol_file
        .meshes
        .iter()
        .map(|p| {
            let (vertices, tex_vertices) = p
                .vertices
                .iter()
                .map(|v| {
                    (
                        Vertex {
                            x: v.position.x as f64,
                            y: v.position.y as f64,
                            z: v.position.z as f64,
                        },
                        TVertex {
                            u: v.tex_coord.u as f64,
                            v: v.tex_coord.v as f64,
                            w: 1.,
                        },
                    )
                })
                .unzip();

            let normals = if p.vertex_type.has(PolVertexComponents::NORMAL) {
                vec![]
            } else {
                vec![]
            };

            let geometry = p
                .material_info
                .iter()
                .map(|m| {
                    let shapes = m
                        .triangles
                        .iter()
                        .map(|t| Shape {
                            primitive: obj_exporter::Primitive::Triangle(
                                (t.indices[0] as usize, None, None),
                                (t.indices[1] as usize, None, None),
                                (t.indices[2] as usize, None, None),
                            ),
                            groups: vec![],
                            smoothing_groups: vec![],
                        })
                        .collect();

                    Geometry {
                        material_name: Some(m.texture_names[0].clone()),
                        shapes,
                    }
                })
                .collect();

            Object {
                name: "".to_string(),
                vertices,
                tex_vertices,
                normals,
                geometry,
            }
        })
        .collect();

    Some(ObjSet {
        material_library: None,
        objects,
    })
}
