//! Owned, fully-resolved UI tree shape that the walker consumes.
//!
//! The Rust walker no longer pokes at `p7::interpreter::context::Data`
//! directly. Instead, the proxy calls `resolve(...)` once per frame to
//! turn the protosept-heap-rooted UiNode struct into this Rust enum,
//! then `walk(&owned, visitor)` does pure tree traversal.

use crosscom_protosept::HostError;
use p7::interpreter::context::{Context, Data};

#[derive(Debug, Clone, PartialEq)]
pub struct OwnedNode {
    pub kind: i64,
    pub label: String,
    pub w: f32,
    pub h: f32,
    pub i1: i64,
    pub i2: i64,
    pub children: Vec<OwnedNode>,
}

/// Materialize a UiNode tree from the protosept heap.
///
/// Errors if `data` doesn't match the UiNode struct shape, if a `box<UiNode>`
/// pointed at by a child is missing, or if any field has the wrong primitive
/// type. Field positions are taken to match `bindings/ui.p7`'s struct layout.
pub fn resolve(ctx: &Context, data: &Data) -> Result<OwnedNode, HostError> {
    let fields = struct_fields(ctx, data)?;
    if fields.len() != 7 {
        return Err(HostError::message(format!(
            "UiNode resolve: expected 7 fields, got {}",
            fields.len()
        )));
    }

    let kind = expect_int(&fields[0], "UiNode.kind")?;
    let label = expect_string(&fields[1], "UiNode.label")?;
    let w = expect_f32(&fields[2], "UiNode.w")?;
    let h = expect_f32(&fields[3], "UiNode.h")?;
    let i1 = expect_int(&fields[4], "UiNode.i1")?;
    let i2 = expect_int(&fields[5], "UiNode.i2")?;
    let children_data = expect_array(ctx, &fields[6], "UiNode.children")?;
    let mut children = Vec::with_capacity(children_data.len());
    for child in children_data {
        children.push(resolve(ctx, child)?);
    }

    Ok(OwnedNode {
        kind,
        label,
        w,
        h,
        i1,
        i2,
        children,
    })
}

fn struct_fields<'a>(ctx: &'a Context, data: &Data) -> Result<&'a [Data], HostError> {
    match data {
        Data::StructRef(idx) => ctx
            .heap
            .get(*idx as usize)
            .map(|s| s.fields.as_slice())
            .ok_or_else(|| HostError::message(format!("UiNode resolve: heap[{idx}] missing"))),
        Data::BoxRef(idx) => match ctx.box_heap.get(*idx as usize) {
            Some(inner) => struct_fields(ctx, inner),
            None => Err(HostError::message(format!(
                "UiNode resolve: box_heap[{idx}] missing"
            ))),
        },
        Data::ProtoBoxRef { box_idx, .. } => match ctx.box_heap.get(*box_idx as usize) {
            Some(inner) => struct_fields(ctx, inner),
            None => Err(HostError::message(format!(
                "UiNode resolve: box_heap[{box_idx}] missing"
            ))),
        },
        other => Err(HostError::message(format!(
            "UiNode resolve: expected struct ref, got {other:?}"
        ))),
    }
}

fn expect_int(data: &Data, name: &str) -> Result<i64, HostError> {
    match data {
        Data::Int(v) => Ok(*v),
        other => Err(HostError::message(format!(
            "{name}: expected int, got {other:?}"
        ))),
    }
}

fn expect_string(data: &Data, name: &str) -> Result<String, HostError> {
    match data {
        Data::String(v) => Ok(v.to_string()),
        other => Err(HostError::message(format!(
            "{name}: expected string, got {other:?}"
        ))),
    }
}

fn expect_f32(data: &Data, name: &str) -> Result<f32, HostError> {
    match data {
        Data::Float(v) => Ok(*v as f32),
        Data::Int(v) => Ok(*v as f32),
        other => Err(HostError::message(format!(
            "{name}: expected float, got {other:?}"
        ))),
    }
}

fn expect_array<'a>(ctx: &'a Context, data: &'a Data, name: &str) -> Result<&'a [Data], HostError> {
    match data {
        Data::Array(items) => Ok(items.as_slice()),
        Data::BoxRef(idx) => match ctx.box_heap.get(*idx as usize) {
            Some(Data::Array(items)) => Ok(items.as_slice()),
            Some(other) => Err(HostError::message(format!(
                "{name}: expected boxed array, got {other:?}"
            ))),
            None => Err(HostError::message(format!(
                "{name}: box_heap[{idx}] missing"
            ))),
        },
        Data::ProtoBoxRef { box_idx, .. } => match ctx.box_heap.get(*box_idx as usize) {
            Some(Data::Array(items)) => Ok(items.as_slice()),
            Some(other) => Err(HostError::message(format!(
                "{name}: expected boxed array, got {other:?}"
            ))),
            None => Err(HostError::message(format!(
                "{name}: box_heap[{box_idx}] missing"
            ))),
        },
        other => Err(HostError::message(format!(
            "{name}: expected array, got {other:?}"
        ))),
    }
}
