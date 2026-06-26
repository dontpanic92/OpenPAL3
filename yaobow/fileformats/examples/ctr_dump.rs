//! Throwaway: dump real .ctr decoded stats to understand grass geometry.
use fileformats::pal5::ctr::CtrFile;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"F:\PAL5\Map\kuangfengzhai\kuangfengzhai_0_0.ctr".to_string());
    let raw = std::fs::read(&path).unwrap();
    let ctr = CtrFile::read(&raw).unwrap();
    println!("depth={} leaves={}", ctr.depth, ctr.leaves.len());

    let with_verts = ctr.leaves.iter().filter(|l| !l.vertices.is_empty()).count();
    let with_tris = ctr.leaves.iter().filter(|l| !l.triangles.is_empty()).count();
    let with_density = ctr.leaves.iter().filter(|l| !l.density.is_empty()).count();
    let total_verts: usize = ctr.leaves.iter().map(|l| l.vertices.len()).sum();
    let total_tris: usize = ctr.leaves.iter().map(|l| l.triangles.len()).sum();
    let total_cells: usize = ctr.leaves.iter().map(|l| l.density.len()).sum();
    println!(
        "with_verts={with_verts} with_tris={with_tris} with_density={with_density}",
    );
    println!("total_verts={total_verts} total_tris={total_tris} total_density_cells={total_cells}");

    // Global vertex bbox (custom geometry only).
    let mut vmin = [f32::MAX; 3];
    let mut vmax = [f32::MIN; 3];
    for l in &ctr.leaves {
        for v in &l.vertices {
            for i in 0..3 {
                vmin[i] = vmin[i].min(v[i]);
                vmax[i] = vmax[i].max(v[i]);
            }
        }
    }
    println!("custom vert bbox: min={vmin:?} max={vmax:?}");

    // g-range distribution.
    let mut g_min = [i32::MAX; 4];
    let mut g_max = [i32::MIN; 4];
    for l in &ctr.leaves {
        for i in 0..4 {
            g_min[i] = g_min[i].min(l.g[i]);
            g_max[i] = g_max[i].max(l.g[i]);
        }
    }
    println!("g min={g_min:?} max={g_max:?}");

    // density value histogram.
    let mut hist = std::collections::BTreeMap::new();
    for l in &ctr.leaves {
        for &d in &l.density {
            *hist.entry(d).or_insert(0usize) += 1;
        }
    }
    println!("density histogram: {hist:?}");

    // Show first few leaves that HAVE custom vertices: their tex, g, vert count,
    // local vertex bbox, and sample verts.
    println!("\n--- first leaves with custom vertices ---");
    for (i, l) in ctr
        .leaves
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.vertices.is_empty())
        .take(6)
    {
        let mut lmin = [f32::MAX; 3];
        let mut lmax = [f32::MIN; 3];
        for v in &l.vertices {
            for k in 0..3 {
                lmin[k] = lmin[k].min(v[k]);
                lmax[k] = lmax[k].max(v[k]);
            }
        }
        println!(
            "leaf#{i} tex0={} tex1={} g={:?} verts={} tris={} density_cells={} vbbox min={:?} max={:?}",
            l.tex0,
            l.tex1,
            l.g,
            l.vertices.len(),
            l.triangles.len(),
            l.density.len(),
            lmin,
            lmax,
        );
        for v in l.vertices.iter().take(6) {
            println!("    v={v:?}");
        }
        for t in l.triangles.iter().take(4) {
            println!("    tri idx={:?} color={:#010x}", t.indices, t.color);
        }
    }

    // Show first few PURE-grid leaves (density but no verts).
    println!("\n--- first pure-grid leaves (density, no verts) ---");
    for (i, l) in ctr
        .leaves
        .iter()
        .enumerate()
        .filter(|(_, l)| l.vertices.is_empty() && !l.density.is_empty())
        .take(6)
    {
        println!(
            "leaf#{i} tex0={} tex1={} g={:?} cols={} rows={} density={:?}",
            l.tex0,
            l.tex1,
            l.g,
            l.cols(),
            l.rows(),
            &l.density[..l.density.len().min(16)],
        );
    }

    // Outlier detection: custom triangles with a very long edge (likely
    // mis-connections that shoot into the sky), and per-leaf Y outliers.
    println!("\n--- long-edge custom triangles (edge > 400 units) ---");
    let mut long_count = 0usize;
    let mut shown = 0;
    for (li, l) in ctr.leaves.iter().enumerate() {
        for t in &l.triangles {
            let idx = t.indices;
            if (idx[0] as usize) >= l.vertices.len()
                || (idx[1] as usize) >= l.vertices.len()
                || (idx[2] as usize) >= l.vertices.len()
            {
                continue;
            }
            let a = l.vertices[idx[0] as usize];
            let b = l.vertices[idx[1] as usize];
            let c = l.vertices[idx[2] as usize];
            let d = |p: [f32; 3], q: [f32; 3]| {
                ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
            };
            let maxedge = d(a, b).max(d(b, c)).max(d(c, a));
            if maxedge > 400.0 {
                long_count += 1;
                if shown < 8 {
                    println!(
                        "leaf#{li} idx={:?} maxedge={:.0} a={:?} b={:?} c={:?}",
                        idx, maxedge, a, b, c
                    );
                    shown += 1;
                }
            }
        }
    }
    println!("total long-edge custom triangles: {long_count}");

    println!("\n--- leaves with large custom-vertex Y span (>250) ---");
    let mut big = 0;
    for (li, l) in ctr.leaves.iter().enumerate() {
        if l.vertices.is_empty() {
            continue;
        }
        let ys: Vec<f32> = l.vertices.iter().map(|v| v[1]).collect();
        let ymin = ys.iter().cloned().fold(f32::MAX, f32::min);
        let ymax = ys.iter().cloned().fold(f32::MIN, f32::max);
        if ymax - ymin > 250.0 {
            big += 1;
            if big <= 8 {
                println!(
                    "leaf#{li} yspan={:.0} ({:.0}..{:.0}) verts={}",
                    ymax - ymin,
                    ymin,
                    ymax,
                    l.vertices.len()
                );
            }
        }
    }
    println!("leaves with Y span > 250: {big}");

    // Find the single most-common vertex position (a shared "cone apex" would
    // show up as one position referenced by many leaves), and the global
    // highest vertex.
    use std::collections::HashMap;
    let mut pos_count: HashMap<(i32, i32, i32), usize> = HashMap::new();
    let mut highest: Option<(f32, [f32; 3], usize)> = None;
    for (li, l) in ctr.leaves.iter().enumerate() {
        for v in &l.vertices {
            let key = (
                (v[0] / 4.0).round() as i32,
                (v[1] / 4.0).round() as i32,
                (v[2] / 4.0).round() as i32,
            );
            *pos_count.entry(key).or_insert(0) += 1;
            if highest.map(|(y, _, _)| v[1] > y).unwrap_or(true) {
                highest = Some((v[1], *v, li));
            }
        }
    }
    let mut top: Vec<_> = pos_count.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\n--- most-common vertex cells (x4,y4,z4 -> count) ---");
    for (k, c) in top.iter().take(8) {
        println!("  ({},{},{}) x4 -> {} refs", k.0, k.1, k.2, c);
    }
    if let Some((y, v, li)) = highest {
        println!("highest vertex: y={:.1} at {:?} in leaf#{}", y, v, li);
    }

    // Correlate triangle color flags with ribbon height: maybe the engine
    // distinguishes tall "spike" ribbons from short grass by a color-flag bit.
    println!("\n--- color-flag vs ribbon height ---");
    use std::collections::BTreeMap;
    let mut by_color: BTreeMap<u32, (usize, f32, usize)> = BTreeMap::new(); // color -> (tris, max_yspan_seen, leaves)
    for l in &ctr.leaves {
        if l.vertices.is_empty() {
            continue;
        }
        let ys: Vec<f32> = l.vertices.iter().map(|v| v[1]).collect();
        let yspan = ys.iter().cloned().fold(f32::MIN, f32::max)
            - ys.iter().cloned().fold(f32::MAX, f32::min);
        let mut seen = std::collections::BTreeSet::new();
        for t in &l.triangles {
            let e = by_color.entry(t.color).or_insert((0, 0.0, 0));
            e.0 += 1;
            e.1 = e.1.max(yspan);
            seen.insert(t.color);
        }
        for c in seen {
            by_color.get_mut(&c).unwrap().2 += 1;
        }
    }
    for (color, (tris, maxspan, leaves)) in &by_color {
        println!(
            "  color={:#010x} (low4={:#x}): {} tris, {} leaves, max ribbon yspan {:.0}",
            color,
            color & 0xf,
            tris,
            leaves,
            maxspan
        );
    }

    // Deep-dump one tall leaf: are the spikes genuine thin triangles in the
    // data, and does one vertex index dominate (a per-leaf shared apex)?
    let li = std::env::args()
        .nth(3)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(126);
    if let Some(l) = ctr.leaves.get(li) {
        println!("\n=== deep dump leaf#{li} ===");
        println!(
            "tex0={} tex1={} g={:?} verts={} tris={} density_cells={}",
            l.tex0,
            l.tex1,
            l.g,
            l.vertices.len(),
            l.triangles.len(),
            l.density.len()
        );
        for (i, v) in l.vertices.iter().enumerate() {
            println!("  v[{i}] = [{:.1}, {:.1}, {:.1}]", v[0], v[1], v[2]);
        }
        // Index usage histogram + per-triangle max edge.
        let mut idx_hist = std::collections::BTreeMap::new();
        for t in &l.triangles {
            for &i in &t.indices {
                *idx_hist.entry(i).or_insert(0usize) += 1;
            }
        }
        println!("  index usage: {idx_hist:?}");
        for (ti, t) in l.triangles.iter().enumerate() {
            let inb = t.indices.iter().all(|&i| (i as usize) < l.vertices.len());
            let edge = if inb {
                let a = l.vertices[t.indices[0] as usize];
                let b = l.vertices[t.indices[1] as usize];
                let c = l.vertices[t.indices[2] as usize];
                let d = |p: [f32; 3], q: [f32; 3]| {
                    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
                };
                d(a, b).max(d(b, c)).max(d(c, a))
            } else {
                -1.0
            };
            println!(
                "  tri[{ti}] idx={:?} color={:#010x} inbounds={} maxedge={:.0}",
                t.indices, t.color, inb, edge
            );
        }
    }
}
