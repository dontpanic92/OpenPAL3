//! PAL5 `MapInfo.ini` per-map config decoder (sun light + scene extent).
//!
//! PAL5 stores its directional sun in `Config/Data.pkg::\MapInfo.ini`, not in
//! `envinfo.env`. Each map is a numbered `[N]` section keyed by `Name=`; the
//! relevant fields are reverse-engineered clean-room from the unpacked
//! `Pal5.exe` map loader (`fcn.0072ef10`, which reads `sunX/sunY/sunZ` into the
//! map struct alongside `Ambient`, `DLit`, `HDRExp`, etc.):
//!
//! ```ini
//! [22]
//! Name=kuangfengzhai
//! MidMapArray=3808,0,1420,5120,6530   ; (cx, _, _, sizeX, sizeZ)
//! Ambient=0.65
//! DLit=0.6
//! ; no sunX/sunY/sunZ -> overhead default
//! ```
//!
//! `sunX/sunY/sunZ` are integer **world-space positions**; the sun direction
//! used for lighting is `normalize((sunX,sunY,sunZ) - mapCenter)` with the map
//! centre taken from `MidMapArray` (sizeX/2, 0, sizeZ/2). Most maps (including
//! `kuangfengzhai`) ship *no* sun fields and fall back to the engine's
//! near-overhead default — which is exactly the "noon" look those maps have.

use std::collections::HashMap;

/// Per-map sun lighting parsed from `MapInfo.ini`.
#[derive(Debug, Clone)]
pub struct MapSun {
    /// Unit direction from the ground toward the sun (engine LH Y-up world),
    /// or `None` when the map ships no `sunX/sunY/sunZ` (use overhead default).
    pub direction: Option<[f32; 3]>,
}

/// Parsed `MapInfo.ini`: per-map name -> sun config.
pub struct MapInfoFile {
    maps: HashMap<String, MapSun>,
}

impl MapInfoFile {
    /// Parse the whole `MapInfo.ini` (GBK/ASCII; only ASCII keys/numbers are
    /// read so lossy UTF-8 is fine). Sections are `[N]`; the map is keyed by
    /// its `Name=` value.
    pub fn parse(text: &str) -> Self {
        let mut maps = HashMap::new();
        let mut name: Option<String> = None;
        let mut sun = [None, None, None];
        let mut center = (2560.0f32, 2560.0f32);

        let mut flush = |name: &mut Option<String>, sun: &mut [Option<f32>; 3], center: (f32, f32)| {
            if let Some(n) = name.take() {
                let direction = match (sun[0], sun[1], sun[2]) {
                    (Some(x), Some(y), Some(z)) => {
                        let v = [x - center.0, y, z - center.1];
                        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                        if len > 1e-3 {
                            Some([v[0] / len, v[1] / len, v[2] / len])
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                maps.insert(n, MapSun { direction });
            }
            *sun = [None, None, None];
        };

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                flush(&mut name, &mut sun, center);
                center = (2560.0, 2560.0);
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "Name" => name = Some(v.to_string()),
                "sunX" => sun[0] = v.parse().ok(),
                "sunY" => sun[1] = v.parse().ok(),
                "sunZ" => sun[2] = v.parse().ok(),
                "MidMapArray" => {
                    let n: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if n.len() >= 5 {
                        center = (n[3] * 0.5, n[4] * 0.5);
                    }
                }
                _ => {}
            }
        }
        flush(&mut name, &mut sun, center);
        Self { maps }
    }

    /// Sun config for a map by `Name`, if listed.
    pub fn sun(&self, map_name: &str) -> Option<&MapSun> {
        self.maps.get(map_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_with_sun_normalizes_relative_to_center() {
        let ini = "[12]\nName=qingmucun\nMidMapArray=6029,780,340,4640,4200\nsunX=4476\nsunY=2120\nsunZ=2403\n";
        let mi = MapInfoFile::parse(ini);
        let d = mi.sun("qingmucun").unwrap().direction.unwrap();
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4);
        assert!(d[1] > 0.4, "sun should be high overhead, got {d:?}");
    }

    #[test]
    fn map_without_sun_has_no_direction() {
        let ini = "[22]\nName=kuangfengzhai\nAmbient=0.65\nDLit=0.6\n";
        let mi = MapInfoFile::parse(ini);
        assert!(mi.sun("kuangfengzhai").unwrap().direction.is_none());
    }
}
