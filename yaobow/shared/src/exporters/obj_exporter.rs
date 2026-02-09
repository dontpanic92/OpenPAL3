/*
Modifications based on https://github.com/Simteract/obj-exporter-rs

The MIT License (MIT)

Copyright (c) 2022 Shengqiu Li
Copyright (c) 2017 Simteract

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
 */

use std::fs;
use std::io::{BufWriter, Result, Write};
use std::path::Path;

pub use obj::{
    Geometry, GroupName, NormalIndex, ObjSet, Object, Primitive, Shape, TVertex, TextureIndex,
    VTNIndex, Vertex, VertexIndex,
};
use wavefront_obj::mtl::{Illumination, Material, MtlSet};
use wavefront_obj::obj;

/// Exports `ObjSet` to given output.
pub fn export<W: Write>(obj_set: &ObjSet, output: &mut W) -> Result<()> {
    Exporter::new(output).export(obj_set)
}

pub fn export_mtl<W: Write>(mtl_set: &MtlSet, output: &mut W) -> Result<()> {
    MtlExporter::new(output).export(mtl_set)
}

/// Exports `ObjSet`to file.
pub fn export_to_file<P: AsRef<Path>>(
    obj_set: &ObjSet,
    mtl_set: &MtlSet,
    obj_path: P,
) -> Result<()> {
    let mtl_path = obj_path.as_ref().parent().and_then(|p| {
        if let Some(mtllib) = &obj_set.material_library {
            let mut path = p.to_owned();
            path.push(mtllib);
            Some(path)
        } else {
            None
        }
    });

    if let Some(mtl_path) = mtl_path {
        let mtl_file = fs::File::create(mtl_path)?;
        let mut buffered = BufWriter::new(mtl_file);
        export_mtl(mtl_set, &mut buffered)?;
    }

    let file = fs::File::create(obj_path)?;
    let mut buffered = BufWriter::new(file);
    export(obj_set, &mut buffered)
}

struct Exporter<'a, W: 'a + Write> {
    output: &'a mut W,
    v_base_id: usize,
    uv_base_id: usize,
    n_base_id: usize,
    current_groups: Vec<GroupName>,
    current_smoothing_groups: Vec<u32>,
}

impl<'a, W: 'a + Write> Exporter<'a, W> {
    fn new(output: &'a mut W) -> Exporter<'a, W> {
        Exporter {
            output,
            v_base_id: 1,
            uv_base_id: 1,
            n_base_id: 1,
            current_groups: DEFAULT_GROUPS.clone(),
            current_smoothing_groups: vec![0],
        }
    }

    fn export(&mut self, obj_set: &ObjSet) -> Result<()> {
        if let Some(mtllib) = &obj_set.material_library {
            write!(self.output, "mtllib {}\n", mtllib)?
        }

        for object in &obj_set.objects {
            self.serialize_object(object)?;
        }
        Ok(())
    }

    fn serialize_object(&mut self, object: &Object) -> Result<()> {
        write!(self.output, "o {}\n", object.name)?;
        self.serialize_vertex_data(object)?;
        for g in &object.geometry {
            self.serialize_geometry(g)?;
        }
        self.update_base_indices(object);
        Ok(())
    }

    fn serialize_vertex_data(&mut self, object: &Object) -> Result<()> {
        for v in &object.vertices {
            self.serialize_vertex(v, "v")?;
        }
        for uv in &object.tex_vertices {
            self.serialize_uv(uv)?;
        }
        for n in &object.normals {
            self.serialize_vertex(n, "vn")?
        }
        Ok(())
    }

    fn serialize_geometry(&mut self, geometry: &Geometry) -> Result<()> {
        if let Some(mtl) = &geometry.material_name {
            write!(self.output, "usemtl {}\n", mtl)?
        }

        for s in &geometry.shapes {
            self.serialize_shape(s)?;
        }
        Ok(())
    }

    fn serialize_vertex(&mut self, v: &Vertex, prefix: &str) -> Result<()> {
        write!(self.output, "{} {:.6} {:.6} {:.6}\n", prefix, v.x, v.y, v.z)
    }

    fn serialize_uv(&mut self, uv: &TVertex) -> Result<()> {
        if uv.w == 0.0 {
            write!(self.output, "vt {:.6} {:.6}\n", uv.u, uv.v)
        } else {
            write!(self.output, "vt {:.6} {:.6} {:.6}\n", uv.u, uv.v, uv.w)
        }
    }

    fn serialize_shape(&mut self, shape: &Shape) -> Result<()> {
        self.update_and_serialize_groups(&shape.groups)?;
        self.update_and_serialize_smoothing_groups(&shape.smoothing_groups)?;
        self.serialize_primitive(&shape.primitive)
    }

    fn update_and_serialize_groups(&mut self, groups: &[GroupName]) -> Result<()> {
        let normalized_groups = groups_or_default(groups);
        if self.current_groups != normalized_groups {
            write!(self.output, "g")?;
            for g in normalized_groups {
                write!(self.output, " {}", g)?;
            }
            writeln!(self.output, "")?;
            self.current_groups = normalized_groups.to_owned();
        }
        Ok(())
    }

    fn update_and_serialize_smoothing_groups(&mut self, smoothing_groups: &[u32]) -> Result<()> {
        let normalized_groups = smoothing_groups_or_default(smoothing_groups);
        if self.current_smoothing_groups != normalized_groups {
            write!(self.output, "s")?;
            for g in normalized_groups {
                write!(self.output, " {}", g)?;
            }
            writeln!(self.output, "")?;
            self.current_smoothing_groups = normalized_groups.to_owned();
        }
        Ok(())
    }

    fn serialize_primitive(&mut self, primitive: &Primitive) -> Result<()> {
        match *primitive {
            Primitive::Point(vtn) => {
                write!(self.output, "p")?;
                self.serialize_vtn(vtn)?;
            }
            Primitive::Line(vtn1, vtn2) => {
                write!(self.output, "l")?;
                self.serialize_vtn(vtn1)?;
                self.serialize_vtn(vtn2)?;
            }
            Primitive::Triangle(vtn1, vtn2, vtn3) => {
                write!(self.output, "f")?;
                self.serialize_vtn(vtn1)?;
                self.serialize_vtn(vtn2)?;
                self.serialize_vtn(vtn3)?;
            }
        }
        writeln!(self.output, "")
    }

    fn serialize_vtn(&mut self, vtn: VTNIndex) -> Result<()> {
        match vtn {
            (vi, None, None) => write!(self.output, " {}", vi + self.v_base_id),
            (vi, Some(ti), None) => write!(
                self.output,
                " {}/{}",
                vi + self.v_base_id,
                ti + self.uv_base_id
            ),
            (vi, Some(ti), Some(ni)) => write!(
                self.output,
                " {}/{}/{}",
                vi + self.v_base_id,
                ti + self.uv_base_id,
                ni + self.n_base_id
            ),
            (vi, None, Some(ni)) => write!(
                self.output,
                " {}//{}",
                vi + self.v_base_id,
                ni + self.n_base_id
            ),
        }
    }

    fn update_base_indices(&mut self, object: &Object) {
        self.v_base_id += object.vertices.len();
        self.uv_base_id += object.tex_vertices.len();
        self.n_base_id += object.normals.len();
    }
}

struct MtlExporter<'a, W: 'a + Write> {
    output: &'a mut W,
}

impl<'a, W: 'a + Write> MtlExporter<'a, W> {
    fn new(output: &'a mut W) -> MtlExporter<'a, W> {
        MtlExporter { output }
    }

    fn export(&mut self, mtl_set: &MtlSet) -> Result<()> {
        for material in &mtl_set.materials {
            self.serialize_material(material)?;
        }
        Ok(())
    }

    fn serialize_material(&mut self, material: &Material) -> Result<()> {
        write!(self.output, "newmtl {}\n", material.name)?;
        write!(self.output, "Ns {}\n", material.specular_coefficient)?;
        write!(
            self.output,
            "Ka {} {} {}\n",
            material.color_ambient.r, material.color_ambient.g, material.color_ambient.b
        )?;

        write!(
            self.output,
            "Kd {} {} {}\n",
            material.color_diffuse.r, material.color_diffuse.g, material.color_diffuse.b
        )?;

        write!(
            self.output,
            "Ks {} {} {}\n",
            material.color_specular.r, material.color_specular.g, material.color_specular.b
        )?;

        if let Some(e) = &material.color_emissive {
            write!(self.output, "Ke {} {} {}\n", e.r, e.g, e.b)?;
        }

        if let Some(d) = &material.optical_density {
            write!(self.output, "Ni {}\n", d)?;
        }

        write!(self.output, "d {}\n", material.alpha)?;

        let illum = match material.illumination {
            Illumination::Ambient => 0,
            Illumination::AmbientDiffuse => 1,
            Illumination::AmbientDiffuseSpecular => 2,
        };

        write!(self.output, "illum {}\n", illum)?;

        if let Some(map) = &material.uv_map {
            write!(self.output, "map_Kd {}\n", map)?;
        }

        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref DEFAULT_GROUPS: Vec<GroupName> = vec!["default".to_owned()];
    static ref DEFAULT_SMOOTHING_GROUPS: Vec<u32> = vec![0];
}

fn groups_or_default(groups: &[GroupName]) -> &[GroupName] {
    if groups.is_empty() || groups[0].is_empty() {
        &DEFAULT_GROUPS
    } else {
        groups
    }
}

fn smoothing_groups_or_default(smoothing_groups: &[u32]) -> &[u32] {
    if smoothing_groups.is_empty() {
        &DEFAULT_SMOOTHING_GROUPS
    } else {
        smoothing_groups
    }
}
