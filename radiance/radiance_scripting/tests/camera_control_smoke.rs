//! A4 smoke test: `wrap_scene_camera` adapts a `ComRc<IScene>` into
//! an `ICameraControl` and the script-visible basis getters round-
//! trip through a script-driven `set_position` + `look_at`.

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance::comdef::IScene;
use radiance::math::Vec3;
use radiance::scene::wrap_scene_camera;
use radiance_scripting::ScriptHost;

const SCRIPT: &str = r#"
import radiance;

// Position then look-at; report the right-axis y component (should
// be ~0 for a level look-at along world X).
pub fn position_camera_and_get_right_y(
    cam: box<radiance.ICameraControl>,
    px: float, py: float, pz: float,
    tx: float, ty: float, tz: float,
) -> float {
    cam.set_position(px, py, pz);
    cam.look_at(tx, ty, tz);
    cam.right_y()
}

// Returns the camera-forward Z component after look-at(0,0,0) from
// (0, 0, 10): forward = normalize(pos - target) = (0,0,1).
pub fn forward_z_after_look_at_origin(cam: box<radiance.ICameraControl>) -> float {
    cam.set_position(0.0, 0.0, 10.0);
    cam.look_at(0.0, 0.0, 0.0);
    cam.forward_z()
}
"#;

fn make_scene() -> ComRc<IScene> {
    // CoreScene::create gives a fully-initialized scene.
    use radiance::scene::CoreScene;
    let scene = CoreScene::create();
    scene.load();
    scene
}

#[test]
fn script_drives_camera_set_position_and_reads_basis() {
    let host = ScriptHost::new();
    host.load_source(SCRIPT).expect("load");

    let scene = make_scene();
    let cam_com = wrap_scene_camera(scene.clone());
    let com_id = host.intern(cam_com);
    let cam_box = host
        .foreign_box("radiance.comdef.ICameraControl", com_id)
        .expect("foreign box");

    let result = host
        .call_returning_data(
            "position_camera_and_get_right_y",
            vec![
                cam_box,
                Data::Float(0.0),
                Data::Float(5.0),
                Data::Float(10.0),
                Data::Float(0.0),
                Data::Float(5.0),
                Data::Float(0.0),
            ],
        )
        .expect("script runs");

    match result {
        Data::Float(ry) => assert!(
            ry.abs() < 1e-3,
            "right_y should be ~0 for level look-at, got {ry}"
        ),
        other => panic!("expected Float, got {other:?}"),
    }

    // Verify host-side state actually moved.
    use radiance::scene::ISceneExt;
    let pos = scene.camera().transform().position();
    assert!(
        (pos.x - 0.0).abs() < 1e-4 && (pos.y - 5.0).abs() < 1e-4 && (pos.z - 10.0).abs() < 1e-4,
        "camera position should be (0, 5, 10), got {:?}",
        pos
    );
    let _ = Vec3::new(0., 0., 0.);
}

#[test]
fn forward_axis_matches_look_at_geometry() {
    let host = ScriptHost::new();
    host.load_source(SCRIPT).expect("load");

    let scene = make_scene();
    let cam_com = wrap_scene_camera(scene);
    let com_id = host.intern(cam_com);
    let cam_box = host
        .foreign_box("radiance.comdef.ICameraControl", com_id)
        .expect("foreign box");

    let result = host
        .call_returning_data("forward_z_after_look_at_origin", vec![cam_box])
        .expect("script runs");

    match result {
        Data::Float(fz) => assert!(
            (fz - 1.0).abs() < 1e-3,
            "forward_z should be ~1.0 for cam at (0,0,10) looking at origin, got {fz}"
        ),
        other => panic!("expected Float, got {other:?}"),
    }
}
