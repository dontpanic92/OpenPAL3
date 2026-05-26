//! Common builder for assembling a single-buffer glTF document + the
//! `.glb` container packer.
//!
//! All exporters share one binary buffer (the BIN chunk). Each
//! "push_*" helper appends raw bytes, creates a `BufferView` over
//! them, and—for typed pushes—creates an `Accessor` over the view.
//! The bin payload is padded to 4 bytes after every push so accessors
//! satisfy glTF's alignment requirements.

use gltf_json::accessor::{ComponentType, GenericComponentType, Type as AccType};
use gltf_json::buffer;
use gltf_json::validation::{Checked, USize64};
use gltf_json::{Accessor, Index, Root};
use serde_json::json;

/// Single-buffer accumulator used while building one `.glb`. After all
/// data is appended, [`Self::pack`] serializes the JSON, finalizes the
/// buffer length, and emits the binary `.glb` container.
pub struct GlbBuilder {
    pub root: Root,
    pub bin: Vec<u8>,
    pub buffer: Index<gltf_json::Buffer>,
}

impl GlbBuilder {
    pub fn new() -> Self {
        let mut root = Root::default();
        root.asset.generator = Some(format!("yaobow {}", env!("CARGO_PKG_VERSION")));
        // Reserve a single buffer; its `byte_length` is filled in at
        // `pack` time. `.glb` containers identify the BIN chunk by
        // having buffer[0] omit its `uri`.
        let buffer = root.push(gltf_json::Buffer {
            byte_length: USize64(0),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        });
        Self {
            root,
            bin: Vec::new(),
            buffer,
        }
    }

    /// Append raw bytes to the BIN blob, pad to 4 bytes, and return a
    /// `BufferView` over the just-written region.
    pub fn push_view(
        &mut self,
        data: &[u8],
        target: Option<buffer::Target>,
    ) -> Index<buffer::View> {
        let offset = self.bin.len();
        self.bin.extend_from_slice(data);
        // glTF requires accessor data to start on a multiple of its
        // component size (max 4 bytes). Pad every chunk.
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        self.root.push(buffer::View {
            buffer: self.buffer,
            byte_length: USize64(data.len() as u64),
            byte_offset: Some(USize64(offset as u64)),
            byte_stride: None,
            target: target.map(Checked::Valid),
            extensions: Default::default(),
            extras: Default::default(),
        })
    }

    /// Push an `f32` vector array (e.g. positions / normals / morph
    /// deltas). `ty` selects scalar / vec2 / vec3. Optionally records
    /// component-wise min/max (required by the spec for POSITION
    /// accessors).
    pub fn push_f32_accessor(
        &mut self,
        data: &[f32],
        ty: AccType,
        with_minmax: bool,
    ) -> Index<Accessor> {
        let components = component_count(ty);
        assert!(data.len() % components == 0);
        let count = data.len() / components;
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let view = self.push_view(&bytes, Some(buffer::Target::ArrayBuffer));

        let (min, max) = if with_minmax && count > 0 {
            let mut mn = vec![f32::INFINITY; components];
            let mut mx = vec![f32::NEG_INFINITY; components];
            for chunk in data.chunks_exact(components) {
                for i in 0..components {
                    if chunk[i] < mn[i] {
                        mn[i] = chunk[i];
                    }
                    if chunk[i] > mx[i] {
                        mx[i] = chunk[i];
                    }
                }
            }
            (Some(json!(mn)), Some(json!(mx)))
        } else {
            (None, None)
        };

        self.root.push(Accessor {
            buffer_view: Some(view),
            byte_offset: Some(USize64(0)),
            count: USize64(count as u64),
            component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
            type_: Checked::Valid(ty),
            min,
            max,
            normalized: false,
            sparse: None,
            extensions: Default::default(),
            extras: Default::default(),
        })
    }

    /// Push a `u32` scalar accessor (typically vertex indices). glTF
    /// supports `u8`/`u16`/`u32` indices; we always emit `u32` for
    /// simplicity — `.glb` size cost is negligible vs. a typical
    /// embedded texture.
    pub fn push_u32_indices(&mut self, data: &[u32]) -> Index<Accessor> {
        let bytes: Vec<u8> = data.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.push_view(&bytes, Some(buffer::Target::ElementArrayBuffer));
        self.root.push(Accessor {
            buffer_view: Some(view),
            byte_offset: Some(USize64(0)),
            count: USize64(data.len() as u64),
            component_type: Checked::Valid(GenericComponentType(ComponentType::U32)),
            type_: Checked::Valid(AccType::Scalar),
            min: None,
            max: None,
            normalized: false,
            sparse: None,
            extensions: Default::default(),
            extras: Default::default(),
        })
    }

    /// Embed an image's raw bytes (PNG or JPEG) into the BIN blob and
    /// return a glTF `Image` index. Caller wraps it in a `Texture` +
    /// `Material` as needed.
    pub fn push_image(&mut self, bytes: &[u8], mime: &str) -> Index<gltf_json::Image> {
        let view = self.push_view(bytes, None);
        self.root.push(gltf_json::Image {
            buffer_view: Some(view),
            mime_type: Some(gltf_json::image::MimeType(mime.to_string())),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        })
    }

    /// Serialize the JSON chunk, fill in the BIN buffer length, and
    /// emit the final binary `.glb` container.
    ///
    /// Layout (glTF 2.0 §4.4):
    /// ```text
    /// [magic="glTF"][version=2][total_len]
    /// [json_len][type="JSON"][json bytes ... padded to 4 with 0x20]
    /// [bin_len ][type="BIN\0"][bin bytes  ... padded to 4 with 0x00]
    /// ```
    pub fn pack(mut self) -> anyhow::Result<Vec<u8>> {
        // Pad BIN to multiple of 4 (already done after each push, but
        // be defensive in case a caller mutates `bin` directly).
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        // Finalize buffer length now that all data is appended.
        self.root.buffers[0].byte_length = USize64(self.bin.len() as u64);

        let json = serde_json::to_vec(&self.root)?;
        let mut json_padded = json;
        while json_padded.len() % 4 != 0 {
            json_padded.push(0x20); // glTF spec: pad JSON with spaces.
        }

        let total = 12 + 8 + json_padded.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);

        // Header.
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());

        // JSON chunk.
        out.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_padded);

        // BIN chunk.
        out.extend_from_slice(&(self.bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&self.bin);

        Ok(out)
    }
}

fn component_count(ty: AccType) -> usize {
    match ty {
        AccType::Scalar => 1,
        AccType::Vec2 => 2,
        AccType::Vec3 => 3,
        AccType::Vec4 => 4,
        AccType::Mat2 => 4,
        AccType::Mat3 => 9,
        AccType::Mat4 => 16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_glb_has_well_formed_header() {
        let glb = GlbBuilder::new().pack().unwrap();
        assert_eq!(&glb[..4], b"glTF");
        let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
        assert_eq!(version, 2);
        let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        assert_eq!(total, glb.len());
        // JSON chunk header.
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        // BIN chunk header (may be empty but must still be present).
        let bin_off = 20 + json_len;
        let bin_len = u32::from_le_bytes(glb[bin_off..bin_off + 4].try_into().unwrap()) as usize;
        assert_eq!(&glb[bin_off + 4..bin_off + 8], b"BIN\0");
        assert_eq!(bin_off + 8 + bin_len, glb.len());
    }

    #[test]
    fn min_max_tracks_extents() {
        let mut b = GlbBuilder::new();
        let acc = b.push_f32_accessor(
            &[0.0, 0.0, 0.0, 1.0, 2.0, 3.0, -1.0, 5.0, 0.5],
            AccType::Vec3,
            true,
        );
        assert_eq!(acc.value(), 0);
        let mn = b.root.accessors[0].min.as_ref().unwrap();
        let mx = b.root.accessors[0].max.as_ref().unwrap();
        assert_eq!(mn, &json!([-1.0, 0.0, 0.0]));
        assert_eq!(mx, &json!([1.0, 5.0, 3.0]));
    }
}
