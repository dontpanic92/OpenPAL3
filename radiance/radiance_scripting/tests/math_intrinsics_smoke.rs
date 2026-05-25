use p7::interpreter::context::Data;
use radiance_scripting::ScriptHost;

#[test]
fn sin_cos_sqrt_intrinsics_compute_floats() {
    let host = ScriptHost::new();
    host.load_source(
        r#"
pub fn sin_of(x: float) -> float { sin(x) }
pub fn cos_of(x: float) -> float { cos(x) }
pub fn sqrt_of(x: float) -> float { sqrt(x) }
pub fn pyth(a: float, b: float) -> float { sqrt(a * a + b * b) }
"#,
    )
    .expect("load");

    let s = host
        .call_returning_data("sin_of", vec![Data::Float(0.0)])
        .unwrap();
    assert!(matches!(s, Data::Float(v) if v.abs() < 1e-9));

    let c = host
        .call_returning_data("cos_of", vec![Data::Float(0.0)])
        .unwrap();
    assert!(matches!(c, Data::Float(v) if (v - 1.0).abs() < 1e-9));

    let r = host
        .call_returning_data("sqrt_of", vec![Data::Float(9.0)])
        .unwrap();
    assert!(matches!(r, Data::Float(v) if (v - 3.0).abs() < 1e-9));

    let p = host
        .call_returning_data("pyth", vec![Data::Float(3.0), Data::Float(4.0)])
        .unwrap();
    assert!(matches!(p, Data::Float(v) if (v - 5.0).abs() < 1e-9));
}
