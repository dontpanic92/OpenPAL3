use std::path::PathBuf;

use super::obj_exporter::{Geometry, ObjSet, Object, Shape, TVertex, Vertex};
use fileformats::pol::{PolFile, PolVertexComponents};
use wavefront_obj::mtl::{Color, Material, MtlSet};

pub fn export_pol_to_obj(pol_file: Option<&PolFile>, name: &str) -> Option<(ObjSet, MtlSet)> {
    if pol_file.is_none() {
        return None;
    }

    let pol_file = pol_file.unwrap();
    let mtllib_name = name.to_string() + ".mtl";
    let objects = convert_obj(pol_file);
    let materials = convert_mtl(pol_file);

    Some((
        ObjSet {
            material_library: Some(mtllib_name),
            objects,
        },
        MtlSet { materials },
    ))
}

fn convert_mtl(pol_file: &PolFile) -> Vec<Material> {
    pol_file
        .meshes
        .iter()
        .flat_map(|p| {
            p.material_info.iter().map(|m| {
                let texture_name = m.texture_names.last().unwrap().clone();
                let texture_path = PathBuf::from(&texture_name).with_extension("dds");
                Material {
                    name: texture_name.clone(),
                    specular_coefficient: 0.078431,
                    color_ambient: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                    },
                    color_diffuse: Color {
                        r: 0.64,
                        g: 0.64,
                        b: 0.64,
                    },
                    color_specular: Color {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                    },
                    color_emissive: None,
                    optical_density: Some(1.0),
                    alpha: 1.0,
                    illumination: wavefront_obj::mtl::Illumination::AmbientDiffuseSpecular,
                    uv_map: Some(texture_path.to_string_lossy().to_string()),
                }
            })
        })
        .collect()
}

fn convert_obj(pol_file: &PolFile) -> Vec<Object> {
    pol_file
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
                        if p.vertex_type.has(PolVertexComponents::TEXCOORD2) {
                            TVertex {
                                u: v.tex_coord2.as_ref().unwrap().u as f64,
                                v: 1. - v.tex_coord2.as_ref().unwrap().v as f64,
                                w: 0.,
                            }
                        } else {
                            TVertex {
                                u: v.tex_coord.u as f64,
                                v: 1. - v.tex_coord.v as f64,
                                w: 0.,
                            }
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
                            primitive: wavefront_obj::obj::Primitive::Triangle(
                                (t.indices[0] as usize, Some(t.indices[0] as usize), None),
                                (t.indices[1] as usize, Some(t.indices[1] as usize), None),
                                (t.indices[2] as usize, Some(t.indices[2] as usize), None),
                            ),
                            groups: vec![],
                            smoothing_groups: vec![],
                        })
                        .collect();

                    Geometry {
                        material_name: Some(m.texture_names.last().unwrap().clone()),
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
        .collect()
}
