//! A2 smoke test: the host-side `wrap_ray_caster` adapter routes a
//! script's `cast_aa_ny(ox, oy, oz)` call through to a real
//! `RayCaster` mesh and surfaces both the hit and miss shapes via
//! the `?float` (NaN-sentinel) return.

use std::rc::Rc;

use p7::interpreter::context::Data;
use radiance::math::Vec3;
use radiance::utils::ray_casting::{wrap_ray_caster, RayCaster};
use radiance_scripting::ScriptHost;

const SCRIPT: &str = r#"
import radiance;

// Hit case: origin above the floor mesh — returns the (Some) depth.
pub fn depth_above(rc: box<radiance.IRayCaster>, ox: float, oy: float, oz: float) -> float {
    let hit: ?float = rc.cast_aa_ny(ox, oy, oz);
    if hit == null {
        return -1.0;
    }
    hit!
}

// Miss case: origin outside the mesh footprint — returns null.
pub fn is_miss(rc: box<radiance.IRayCaster>, ox: float, oy: float, oz: float) -> bool {
    let hit: ?float = rc.cast_aa_ny(ox, oy, oz);
    hit == null
}
"#;

fn floor_caster() -> Rc<RayCaster> {
    // Two-triangle 10×10 floor at y=0, centered on origin.
    let mut rc = RayCaster::new();
    let verts = vec![
        Vec3::new(-5.0, 0.0, -5.0),
        Vec3::new(5.0, 0.0, -5.0),
        Vec3::new(5.0, 0.0, 5.0),
        Vec3::new(-5.0, 0.0, 5.0),
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    rc.add_mesh(verts, indices);
    Rc::new(rc)
}

#[test]
fn script_observes_some_depth_when_origin_is_above_floor() {
    let host = ScriptHost::new();
    host.load_source(SCRIPT).expect("load");

    let rc_com = wrap_ray_caster(floor_caster());
    let com_id = host.intern(rc_com);
    let rc_box = host
        .foreign_box("radiance.comdef.IRayCaster", com_id)
        .expect("foreign box");

    let depth = host
        .call_returning_data(
            "depth_above",
            vec![rc_box, Data::Float(0.0), Data::Float(7.5), Data::Float(0.0)],
        )
        .expect("depth_above runs");

    match depth {
        Data::Float(d) => assert!(
            (d - 7.5).abs() < 1e-4,
            "expected ~7.5 depth from y=7.5 to y=0 floor, got {d}"
        ),
        other => panic!("expected Float, got {other:?}"),
    }
}

#[test]
fn script_observes_null_when_ray_misses_floor() {
    let host = ScriptHost::new();
    host.load_source(SCRIPT).expect("load");

    let rc_com = wrap_ray_caster(floor_caster());
    let com_id = host.intern(rc_com);
    let rc_box = host
        .foreign_box("radiance.comdef.IRayCaster", com_id)
        .expect("foreign box");

    let miss = host
        .call_returning_data(
            "is_miss",
            vec![
                rc_box,
                Data::Float(100.0),
                Data::Float(7.5),
                Data::Float(100.0),
            ],
        )
        .expect("is_miss runs");

    assert_eq!(miss, Data::Int(1), "expected true (1) for off-mesh origin");
}
