//! Pure-Rust decoders for PAL3 scene-lighting asset formats.
//!
//! Each PAL3 scene block ships a `<index>.lgt` / `<index>.dkl` / `<index>.DKM`
//! triple alongside its `.scn` / `.pol` data:
//!
//! * [`lgt`] — dynamic light source table (omni point lights).
//! * [`dkl`] — baked per-atomic vertex lighting ("dark light").
//! * [`dkm`] — per-material surface lighting coefficients ("DARK" magic).
//!
//! See `generated/pal3_scn.md` for the reverse-engineering write-up.

pub mod lgt;
// pub mod dkl;  // added in the static-scenery baked-lighting phase
// pub mod dkm;  // added in the static-scenery baked-lighting phase
