//! Throwaway analyzer for PAL3 .scn scene definition files.
//! Mounts every scene CPK, finds .scn files, and performs a per-dword
//! statistical analysis of the fixed-size Role and Node records so we can
//! see which currently-"unknown" offsets actually carry data and guess
//! their type (int enum vs float vs color vs padding).

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, ReadBytesExt};
use mini_fs::{MiniFs, StoreExt};
use packfs::cpk::CpkFs;

const ROLE_SIZE: usize = 0x1C8; // 456
const NODE_SIZE: usize = 0x26C; // 620

struct Header {
    role_num: u16,
    role_offset: u32,
    node_num: u16,
    node_offset: u32,
}

fn read_header(buf: &[u8]) -> Option<Header> {
    if buf.len() < 0x7E || &buf[0..4] != b"SCN\0" {
        return None;
    }
    let mut c = &buf[4..];
    let _magic2 = c.read_u16::<LittleEndian>().ok()?;
    let role_num = c.read_u16::<LittleEndian>().ok()?;
    let role_offset = c.read_u32::<LittleEndian>().ok()?;
    let node_num = c.read_u16::<LittleEndian>().ok()?;
    let node_offset = c.read_u32::<LittleEndian>().ok()?;
    Some(Header {
        role_num,
        role_offset,
        node_num,
        node_offset,
    })
}

#[derive(Default, Clone)]
struct DwordStat {
    nonzero: u32,
    total: u32,
    distinct_u32: BTreeSet<u32>,
    // float interpretation range (only for finite, "reasonable" floats)
    fmin: f32,
    fmax: f32,
    float_like: u32,
    int_small: u32, // values that look like small ints / enums (<4096)
}

impl DwordStat {
    fn add(&mut self, v: u32) {
        self.total += 1;
        if v != 0 {
            self.nonzero += 1;
        }
        if self.distinct_u32.len() < 24 {
            self.distinct_u32.insert(v);
        }
        let f = f32::from_bits(v);
        if f.is_finite() && f != 0.0 && f.abs() > 1e-6 && f.abs() < 1e7 {
            self.float_like += 1;
            if self.fmin == 0.0 && self.fmax == 0.0 {
                self.fmin = f;
                self.fmax = f;
            } else {
                self.fmin = self.fmin.min(f);
                self.fmax = self.fmax.max(f);
            }
        }
        if v != 0 && v < 4096 {
            self.int_small += 1;
        }
    }
}

fn analyze(records: &[Vec<u8>], rec_size: usize, label: &str, known: &dyn Fn(usize) -> Option<&'static str>) {
    let ndw = rec_size / 4;
    let mut stats = vec![DwordStat::default(); ndw];
    for r in records {
        if r.len() < rec_size {
            continue;
        }
        for d in 0..ndw {
            let off = d * 4;
            let v = u32::from_le_bytes([r[off], r[off + 1], r[off + 2], r[off + 3]]);
            stats[d].add(v);
        }
    }

    println!("\n================ {label}  ({} records) ================", records.len());
    println!("{:>5} {:>5} {:>10} {:>8} {:>8}  {:<22} interpretation / distinct", "off", "hex", "nonzero%", "floats", "smallint", "field");
    for d in 0..ndw {
        let s = &stats[d];
        if s.total == 0 {
            continue;
        }
        let off = d * 4;
        let nz_pct = 100.0 * s.nonzero as f32 / s.total as f32;
        let kname = known(off).unwrap_or("?");
        // skip dwords that are fully inside a known string field to reduce noise
        let mut interp = String::new();
        if s.nonzero == 0 {
            interp.push_str("ZERO (padding?)");
        } else {
            let distinct: Vec<u32> = s.distinct_u32.iter().take(10).cloned().collect();
            let looks_float = s.float_like as f32 / s.nonzero.max(1) as f32 > 0.7;
            let looks_enum = s.distinct_u32.len() <= 12 && s.int_small as f32 / s.nonzero.max(1) as f32 > 0.7;
            if looks_float {
                interp.push_str(&format!("FLOAT [{:.3}..{:.3}]", s.fmin, s.fmax));
            } else if looks_enum {
                interp.push_str(&format!("ENUM/int distinct={:?}", distinct));
            } else {
                let dvals: Vec<String> = distinct.iter().map(|v| format!("{:#x}", v)).collect();
                interp.push_str(&format!("MIXED n_distinct={} {:?}", s.distinct_u32.len(), dvals));
            }
        }
        println!("{off:>5} {off:>#5x} {nz_pct:>9.1}% {:>8} {:>8}  {kname:<22} {interp}", s.float_like, s.int_small);
    }
}

fn node_known(off: usize) -> Option<&'static str> {
    Some(match off {
        0x00 => "index/w2",
        0x04..=0x23 => "name[32]",
        0x24 => "w24/w26",
        0x28 => "position.x",
        0x2c => "position.y",
        0x30 => "position.z",
        0x34 => "rotation",
        0x38 => "navTrigMin.x",
        0x3c => "navTrigMin.z",
        0x40 => "navTrigMax.x",
        0x44 => "navTrigMax.z",
        0x48 => "type/navLayer",
        0x4c => "ladderC1.x",
        0x50 => "ladderC1.z",
        0x54 => "ladderC2.x",
        0x58 => "ladderC2.z",
        0x5c => "ladderSwitchLayer",
        0x80 => "sceProcId",
        0x16c => "aabb1.x",
        0x170 => "aabb1.y",
        0x174 => "aabb1.z",
        0x178 => "aabb2.x",
        0x17c => "aabb2.y",
        0x180 => "aabb2.z",
        _ => return None,
    })
}

fn role_known(off: usize) -> Option<&'static str> {
    Some(match off {
        0x00 => "index/b1",
        0x02..=0x41 => "name[64]",
        0x42 => "w42",
        0x44 => "dw44(f32)",
        0x48 => "dw48",
        0x4c => "position.x",
        0x50 => "position.z",
        0x54 => "position.y",
        0x5c => "sceProcId",
        0x64..=0x73 => "actionName[16]",
        _ => return None,
    })
}

fn collect_scn(vfs: &MiniFs, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    collect_ext(vfs, dir, ".scn", out)
}

fn collect_ext(vfs: &MiniFs, dir: &Path, ext: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let entries = match vfs.entries(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = dir.join(&entry.name);
        match entry.kind {
            mini_fs::EntryKind::Dir => collect_ext(vfs, &p, ext, out),
            mini_fs::EntryKind::File => {
                if entry.name.to_string_lossy().to_lowercase().ends_with(ext) {
                    if let Ok(mut f) = vfs.open(&p) {
                        let mut buf = vec![];
                        if f.read_to_end(&mut buf).is_ok() {
                            out.push((p.to_string_lossy().into_owned(), buf));
                        }
                    }
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let scene_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/Users/dontpanic/data/Pal3/scene".to_string());

    let mut all_roles: Vec<Vec<u8>> = vec![];
    let mut all_nodes: Vec<Vec<u8>> = vec![];
    let mut file_count = 0;
    let mut night_count = 0;
    let mut skybox_ids = BTreeSet::new();

    for ent in std::fs::read_dir(&scene_dir)? {
        let path = ent?.path();
        if path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some("cpk".into()) {
            continue;
        }
        let fs = match CpkFs::new(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("skip {}: {e}", path.display());
                continue;
            }
        };
        let mut vfs = MiniFs::new(false);
        vfs = vfs.mount("/", fs);
        if let Ok(want) = std::env::var("TREE_LIST") {
            if path.file_name().and_then(|f| f.to_str()).map(|f| f.to_lowercase().contains(&want.to_lowercase())).unwrap_or(false) {
                fn walk(vfs: &MiniFs, dir: &Path, depth: usize) {
                    if let Ok(entries) = vfs.entries(dir) {
                        let mut v: Vec<_> = entries.flatten().collect();
                        v.sort_by_key(|e| e.name.clone());
                        for e in v {
                            let p = dir.join(&e.name);
                            let sz = vfs.open(&p).ok().and_then(|mut f| { let mut b=vec![]; std::io::Read::read_to_end(&mut f,&mut b).ok().map(|_| b.len()) });
                            println!("{}{} {}", "  ".repeat(depth), e.name.to_string_lossy(), sz.map(|s| format!("({s})")).unwrap_or_default());
                            if matches!(e.kind, mini_fs::EntryKind::Dir) { walk(vfs, &p, depth+1); }
                        }
                    }
                }
                walk(&vfs, Path::new("/"), 0);
            }
            continue;
        }

        if std::env::var("DKL_DECODE").is_ok() {
            let mut files = vec![];
            collect_ext(&vfs, Path::new("/"), ".dkl", &mut files);
            for (n, b) in files.into_iter().take(2) {
                let u = |o: usize| u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                let count = u(0x08) as usize;
                let name_table_end = 0x0c + count*64;
                println!("\n{n} len={} count={} name_table_end={:#x}", b.len(), count, name_table_end);
                // first few names
                for i in 0..count.min(4) {
                    let o = 0x0c + i*64;
                    let name: String = b[o..o+32].iter().take_while(|&&c| c!=0 && c!=0xcc).map(|&c| c as char).collect();
                    println!("  atom[{i}] = {name:?}");
                }
                // dump bytes right after name table
                let start = name_table_end;
                for row in 0..16 {
                    let o = start + row*16;
                    if o+16 > b.len() { break; }
                    let hex: Vec<String> = b[o..o+16].iter().map(|x| format!("{x:02x}")).collect();
                    let f: Vec<String> = (0..4).map(|i| format!("{:>10.3}", f32::from_le_bytes([b[o+i*4],b[o+i*4+1],b[o+i*4+2],b[o+i*4+3]]))).collect();
                    let asc: String = b[o..o+16].iter().map(|&x| if (32..127).contains(&x){x as char}else{'.'}).collect();
                    println!("  {o:#08x}  {}  {asc}  | u0={} | {}", hex.join(" "), u(o), f.join(" "));
                }
            }
            return Ok(());
        }
        if std::env::var("SCN_HEADER").is_ok() {
            let mut scns=vec![]; collect_ext(&vfs, Path::new("/"), ".scn", &mut scns);
            for (n,b) in scns.into_iter().take(4) {
                if b.len()<0x90 || &b[0..4]!=b"SCN\0" { continue; }
                let u16a=|o:usize| u16::from_le_bytes([b[o],b[o+1]]);
                let u32a=|o:usize| u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                let role_off=u32a(0x8); let node_off=u32a(0xe);
                println!("{n}: role_num={} role_off={:#x} node_num={} node_off={:#x} gidx={} night={} sky={}", u16a(6), role_off, u16a(0xc), node_off, u32a(0x72), u32a(0x76), u32a(0x7a));
                // dump bytes 0x7e .. role_off (the unparsed global region)
                let end=(role_off as usize).min(b.len());
                let mut o=0x7e;
                while o+16<=end {
                    let hex:Vec<String>=b[o..o+16].iter().map(|x| format!("{x:02x}")).collect();
                    let f:Vec<String>=(0..4).map(|i| format!("{:>10.3}", f32::from_le_bytes([b[o+i*4],b[o+i*4+1],b[o+i*4+2],b[o+i*4+3]]))).collect();
                    println!("   {o:#06x}  {}  | {}", hex.join(" "), f.join(" "));
                    o+=16;
                }
                println!("   (global region 0x7e..{:#x} = {} bytes)", role_off, role_off as usize-0x7e);
            }
            return Ok(());
        }
        if let Ok(cpkpath)=std::env::var("MV3_NORM_CPK") {
            let mut processed=0;
            // Mount an arbitrary cpk (e.g. basedata) and analyze the first MV3's normals.
            let fs2 = packfs::cpk::CpkFs::new(&cpkpath).unwrap();
            let mut vfs2 = MiniFs::new(false); vfs2 = vfs2.mount("/", fs2);
            let mut mv3s=vec![]; collect_ext(&vfs2, Path::new("/"), ".mv3", &mut mv3s);
            let want=std::env::var("MV3_NORM").unwrap_or_default();
            for (n,b) in mv3s {
                if !want.is_empty() && !n.to_lowercase().contains(&want.to_lowercase()) { continue; }
                let mv3=match fileformats::mv3::read_mv3(&mut std::io::Cursor::new(&b)){Ok(x)=>x,Err(_)=>continue};
                let model=&mv3.models[0]; let mesh=&model.meshes[0]; let frame=&model.frames[0];
                // build geometric normals over the raw frame vertices (loader negates x,z)
                let verts: Vec<(f32,f32,f32)> = frame.vertices.iter().map(|v| (v.x as f32, v.y as f32, v.z as f32)).collect();
                let mut gnorm=vec![(0f32,0f32,0f32); verts.len()];
                for t in &mesh.triangles {
                    let (a,bb,c)=(t.indices[0] as usize,t.indices[1] as usize,t.indices[2] as usize);
                    let e1=(verts[bb].0-verts[a].0,verts[bb].1-verts[a].1,verts[bb].2-verts[a].2);
                    let e2=(verts[c].0-verts[a].0,verts[c].1-verts[a].1,verts[c].2-verts[a].2);
                    let cr=(e1.1*e2.2-e1.2*e2.1, e1.2*e2.0-e1.0*e2.2, e1.0*e2.1-e1.1*e2.0);
                    for &idx in &[a,bb,c]{ gnorm[idx].0+=cr.0; gnorm[idx].1+=cr.1; gnorm[idx].2+=cr.2; }
                }
                for g in gnorm.iter_mut(){ let l=(g.0*g.0+g.1*g.1+g.2*g.2).sqrt(); if l>1e-6 {g.0/=l;g.1/=l;g.2/=l;} }
                // decode authored phi/theta with several candidate formulas, score vs geometric
                println!("{n}: verts={} tris={}", verts.len(), mesh.triangles.len());
                // Mesh centroid
                let mut c=(0f32,0f32,0f32); for v in &verts { c.0+=v.0;c.1+=v.1;c.2+=v.2; }
                let nv=verts.len() as f32; c=(c.0/nv,c.1/nv,c.2/nv);
                // Per-FACE winding test: does the face normal point away from centroid?
                let mut out=0; let mut inw=0;
                for t in &mesh.triangles {
                    let (a,bb,cc)=(t.indices[0] as usize,t.indices[1] as usize,t.indices[2] as usize);
                    let e1=(verts[bb].0-verts[a].0,verts[bb].1-verts[a].1,verts[bb].2-verts[a].2);
                    let e2=(verts[cc].0-verts[a].0,verts[cc].1-verts[a].1,verts[cc].2-verts[a].2);
                    let cr=(e1.1*e2.2-e1.2*e2.1, e1.2*e2.0-e1.0*e2.2, e1.0*e2.1-e1.1*e2.0);
                    let fc=((verts[a].0+verts[bb].0+verts[cc].0)/3.0-c.0,(verts[a].1+verts[bb].1+verts[cc].1)/3.0-c.1,(verts[a].2+verts[bb].2+verts[cc].2)/3.0-c.2);
                    let d=cr.0*fc.0+cr.1*fc.1+cr.2*fc.2;
                    if d>=0.0 {out+=1;} else {inw+=1;}
                }
                // Manifold winding test: each interior edge should be traversed in
                // OPPOSITE directions by its two triangles (consistent winding).
                use std::collections::HashMap as HM2;
                let mut edge: HM2<(usize,usize), i32> = HM2::new();
                for t in &mesh.triangles {
                    let idx=[t.indices[0] as usize,t.indices[1] as usize,t.indices[2] as usize];
                    for e in 0..3 {
                        let (a,bb)=(idx[e], idx[(e+1)%3]);
                        let key=(a.min(bb), a.max(bb));
                        let dir = if a<bb {1} else {-1};
                        *edge.entry(key).or_insert(0) += dir;
                    }
                }
                let (mut consistent, mut inconsistent, mut boundary)=(0,0,0);
                for (_,v) in &edge { if *v==0 {consistent+=1;} else if v.abs()==1 {boundary+=1;} else {inconsistent+=1;} }
                println!("  manifold winding: consistent_edges={} inconsistent={} boundary={}", consistent, inconsistent, boundary);
                // Brute force encoding in the SAME space as create_geometry_frames
                // (parser already negated x,z), scored against geometric normals.
                use std::f32::consts::PI;
                let perms: [[usize; 3]; 6] =
                    [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
                let mut best = (-2f32, String::new(), 0usize, 0usize);
                for incl_from_theta in [true, false] {
                    let base: Vec<(f32, f32, f32)> = frame
                        .vertices
                        .iter()
                        .map(|v| {
                            let (ai, aj) = if incl_from_theta {
                                (v.normal_theta as f32, v.normal_phi as f32)
                            } else {
                                (v.normal_phi as f32 + 128.0, v.normal_theta as f32)
                            };
                            let incl = ai * PI / 256.0;
                            let azi = aj * 2.0 * PI / 256.0;
                            (incl.sin() * azi.cos(), incl.cos(), incl.sin() * azi.sin())
                        })
                        .collect();
                    for perm in &perms {
                        for signs in 0..8u8 {
                            let sg = [
                                if signs & 1 != 0 { -1.0 } else { 1.0 },
                                if signs & 2 != 0 { -1.0 } else { 1.0 },
                                if signs & 4 != 0 { -1.0 } else { 1.0 },
                            ];
                            let (mut sum, mut cnt, mut ag, mut fl) = (0f32, 0usize, 0usize, 0usize);
                            for (i, bv) in base.iter().enumerate() {
                                if i >= gnorm.len() {
                                    break;
                                }
                                let arr = [bv.0, bv.1, bv.2];
                                let d = (arr[perm[0]] * sg[0], arr[perm[1]] * sg[1], arr[perm[2]] * sg[2]);
                                let dot = d.0 * gnorm[i].0 + d.1 * gnorm[i].1 + d.2 * gnorm[i].2;
                                sum += dot;
                                cnt += 1;
                                if dot > 0.7 {
                                    ag += 1;
                                } else if dot < -0.7 {
                                    fl += 1;
                                }
                            }
                            let avg = sum / cnt.max(1) as f32;
                            if avg > best.0 {
                                best = (
                                    avg,
                                    format!("incl_from_theta={} perm={:?} signs={:?}", incl_from_theta, perm, sg),
                                    ag,
                                    fl,
                                );
                            }
                        }
                    }
                }
                println!("  BEST avg={:.3} agree={} flip={} :: {}", best.0, best.2, best.3, best.1);
                processed += 1;
                if processed >= 6 {
                    return Ok(());
                }
                continue;
            }
            return Ok(());
        }
        if std::env::var("LM_STATS").is_ok() {
            let mut files=vec![]; collect_ext(&vfs, Path::new("/"), ".dds", &mut files);
            let mut n_lm=0;
            for (n,b) in files {
                let fname=std::path::Path::new(&n).file_name().unwrap().to_string_lossy().to_lowercase();
                if !fname.starts_with("^l_") { continue; }
                if let Ok(img)=image::load_from_memory(&b) {
                    let rgb=img.to_rgb8(); let (mut r,mut g,mut bl)=(0u64,0u64,0u64); let px=rgb.pixels().len() as u64;
                    for p in rgb.pixels(){ r+=p[0] as u64; g+=p[1] as u64; bl+=p[2] as u64; }
                    println!("{} avg=({:.1},{:.1},{:.1}) {}x{}", fname, r as f64/px as f64, g as f64/px as f64, bl as f64/px as f64, rgb.width(), rgb.height());
                    n_lm+=1; if n_lm>=15 { return Ok(()); }
                }
            }
            return Ok(());
        }
        if let Ok(want)=std::env::var("LGT_ONE") {
            let mut files=vec![]; collect_ext(&vfs, Path::new("/"), ".lgt", &mut files);
            for (n,b) in files {
                if !n.to_lowercase().contains(&want.to_lowercase()) { continue; }
                if b.len()<4 {continue;}
                let count=u32::from_le_bytes([b[0],b[1],b[2],b[3]]) as usize;
                let f=|o:usize| f32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                println!("{n} count={}", count);
                for i in 0..count { let o=4+i*148;
                    if o+0x54>b.len(){break;}
                    let pos=(f(o+0x30),f(o+0x34),f(o+0x38));
                    let col=(f(o+0x44),f(o+0x48),f(o+0x4c));
                    let warm = col.0 - col.2;
                    println!("  L{i}: pos=({:.0},{:.0},{:.0}) color=({:.3},{:.3},{:.3}) R-B={:+.3} {}", pos.0,pos.1,pos.2, col.0,col.1,col.2, warm, if warm>0.0 {"WARM"} else {"cool"});
                }
            }
            continue;
        }
        if std::env::var("LGT_WARMTH").is_ok() {
            let mut files=vec![]; collect_ext(&vfs, Path::new("/"), ".lgt", &mut files);
            for (n,b) in files {
                if b.len()<4 {continue;}
                let count=u32::from_le_bytes([b[0],b[1],b[2],b[3]]) as usize;
                let f=|o:usize| f32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                let mut maxc=(0f32,0f32,0f32); let mut sum=(0f32,0f32,0f32);
                for i in 0..count { let o=4+i*148;
                    if o+0x50>b.len(){break;}
                    let c=(f(o+0x44),f(o+0x48),f(o+0x4c));
                    sum.0+=c.0; sum.1+=c.1; sum.2+=c.2;
                    if c.0+c.1+c.2 > maxc.0+maxc.1+maxc.2 { maxc=c; }
                }
                println!("{n} cnt={} brightest=({:.3},{:.3},{:.3}) avg=({:.3},{:.3},{:.3})", count, maxc.0,maxc.1,maxc.2, sum.0/count as f32, sum.1/count as f32, sum.2/count as f32);
            }
            return Ok(());
        }
        if std::env::var("LGT_DECODE").is_ok() {
            let mut lgts = vec![];
            collect_ext(&vfs, Path::new("/"), ".lgt", &mut lgts);
            for (n, b) in lgts {
                if b.len() < 4 { continue; }
                let count = u32::from_le_bytes([b[0],b[1],b[2],b[3]]) as usize;
                let f = |o: usize| f32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                let u = |o: usize| u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                for i in 0..count {
                    let o = 4 + i*148;
                    if o + 148 > b.len() { break; }
                    // matrix row3 translation at o+0x30..0x3c
                    let pos = (f(o+0x30), f(o+0x34), f(o+0x38));
                    let m33 = f(o+0x3c);
                    let c0 = f(o+0x40); let c1=f(o+0x44); let c2=f(o+0x48); let c3=f(o+0x4c);
                    let t1 = u(o+0x64); let t2 = u(o+0x68);
                    let r1 = f(o+0x74); let r2 = f(o+0x78);
                    let cone = f(o+0x7c); let dir=(f(o+0x88),f(o+0x8c),f(o+0x90));
                    println!("{n} L{i:02} pos=({:.0},{:.0},{:.0}) m33={:.2} col4=[{:.3},{:.3},{:.3},{:.3}] t=({},{}) rng=({:.1e},{:.1e}) cone={:.1} dir=({:.2},{:.2},{:.2})",
                        pos.0,pos.1,pos.2,m33,c0,c1,c2,c3,t1,t2,r1,r2,cone,dir.0,dir.1,dir.2);
                }
            }
            return Ok(());
        }
        if std::env::var("DKL_FINDVERT").is_ok() {
            let want=std::env::var("DKL_FINDVERT").unwrap();
            // load POL + its dkl, take first few POL vertex positions, search their byte patterns in dkl
            let mut pols=vec![]; collect_ext(&vfs, Path::new("/"), ".pol", &mut pols);
            for (pn,pb) in pols {
                if !pn.to_lowercase().contains(&want.to_lowercase()) { continue; }
                let mut cur=std::io::Cursor::new(&pb);
                let pf=match fileformats::pol::read_pol(&mut cur){Ok(x)=>x,Err(_)=>continue};
                let base=std::path::Path::new(&pn); let stem=base.file_stem().unwrap().to_string_lossy().to_string(); let dir=base.parent().unwrap();
                let read=|q:std::path::PathBuf| vfs.open(&q).ok().and_then(|mut f|{let mut b=vec![];std::io::Read::read_to_end(&mut f,&mut b).ok().map(|_|b)});
                let dkl=match read(dir.join(format!("{stem}.dkl"))).or_else(||read(dir.join(format!("{stem}.DKL")))){Some(x)=>x,None=>continue};
                println!("== {pn} meshes={} dkl_len={} ==", pf.meshes.len(), dkl.len());
                let find=|needle:&[u8], from:usize| -> Option<usize> { let mut i=from; while i+needle.len()<=dkl.len(){ if &dkl[i..i+needle.len()]==needle {return Some(i);} i+=1; } None };
                // first mesh, first 6 vertices
                let m=&pf.meshes[0];
                let mut hits=vec![];
                for (vi,v) in m.vertices.iter().take(8).enumerate() {
                    let mut nb=vec![]; nb.extend_from_slice(&v.position.x.to_le_bytes()); nb.extend_from_slice(&v.position.y.to_le_bytes()); nb.extend_from_slice(&v.position.z.to_le_bytes());
                    if let Some(off)=find(&nb,0) { println!("  POLv{vi} ({:.2},{:.2},{:.2}) -> dkl @ {:#x}", v.position.x,v.position.y,v.position.z, off); hits.push(off); }
                    else { println!("  POLv{vi} ({:.2},{:.2},{:.2}) -> NOT FOUND (xyz contiguous)", v.position.x,v.position.y,v.position.z); }
                }
                if hits.len()>=2 { let d:Vec<i64>=hits.windows(2).map(|w| w[1] as i64-w[0] as i64).collect(); println!("  hit deltas: {:?}", d); }
                // single-float search for first mesh vertex X / Y / Z
                let v=&m.vertices[0];
                for (lbl,val) in [("x",v.position.x),("y",v.position.y),("z",v.position.z)] {
                    let nb=val.to_le_bytes(); println!("  single {} {:.3} -> {:?}", lbl, val, find(&nb,0).map(|o| format!("{:#x}",o)));
                }
                // triangle total
                let tris: usize = pf.meshes.iter().map(|mm| mm.material_info.iter().map(|mi| mi.triangles.len()).sum::<usize>()).sum();
                let verts: usize = pf.meshes.iter().map(|mm| mm.vertices.len()).sum();
                // tail size estimate: after first 0xffffffff marker
                let mut t0=0; { let mut i=0; while i+4<=dkl.len(){ if dkl[i]==0xff&&dkl[i+1]==0xff&&dkl[i+2]==0xff&&dkl[i+3]==0xff { t0=i; break;} i+=1; } }
                let tail=dkl.len()-t0;
                println!("  tris={} verts={} tail_from_firstFFFF({:#x})={} | /vert={:.2} /tri={:.2} /3tri={:.2}", tris, verts, t0, tail, tail as f64/verts as f64, tail as f64/tris as f64, tail as f64/(3*tris) as f64);
                // search first mesh vertex normals
                if let Some(nrm)=&m.vertices[0].normal {
                    let mut nb=vec![]; nb.extend_from_slice(&nrm.x.to_le_bytes()); nb.extend_from_slice(&nrm.y.to_le_bytes()); nb.extend_from_slice(&nrm.z.to_le_bytes());
                    println!("  POLv0 normal ({:.3},{:.3},{:.3}) contiguous -> {:?}", nrm.x,nrm.y,nrm.z, find(&nb,0).map(|o| format!("{:#x}",o)));
                    println!("  POLv0 normal.x alone -> {:?}", find(&nrm.x.to_le_bytes(),0).map(|o| format!("{:#x}",o)));
                }
                // dump 8 records of 32 bytes from t0 (skip the marker run), interpreting floats at +0,+4,...
                let start=t0+0x90;
                for r in 0..10 { let o=start+r*32; if o+32>dkl.len(){break;}
                    let fl:Vec<String>=(0..8).map(|i| format!("{:>9.3}", f32::from_le_bytes([dkl[o+i*4],dkl[o+i*4+1],dkl[o+i*4+2],dkl[o+i*4+3]]))).collect();
                    println!("    rec@{:#x}: {}", o, fl.join(" "));
                }
            }
            return Ok(());
        }
        if std::env::var("DKL_STRIDE").is_ok() {
            let want=std::env::var("DKL_STRIDE").unwrap();
            let mut files=vec![]; collect_ext(&vfs, Path::new("/"), ".dkl", &mut files);
            // also need POL counts
            for (n,b) in files {
                if !n.to_lowercase().contains(&want.to_lowercase()) { continue; }
                let u=|o:usize| u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]) as usize;
                let cnt=u(0x8); let palstart=0xc+cnt*64;
                // find end of palette = last Art-xufei + its string, approximate: scan to first long run with 0xffffffff markers
                // autocorrelation of byte equality over candidate strides in tail
                let tail0 = {
                    // find first 0xffffffff occurrence after palstart+0x400
                    let mut i=palstart; let mut found=palstart;
                    while i+4<=b.len(){ if b[i]==0xff&&b[i+1]==0xff&&b[i+2]==0xff&&b[i+3]==0xff { found=i; break; } i+=1; }
                    found
                };
                let mut best=(0usize,0f64);
                for stride in 16..=128 {
                    let mut match_cnt=0u64; let mut total=0u64;
                    let mut i=tail0;
                    while i+stride < b.len() && total < 20000 {
                        if b[i]==b[i+stride] { match_cnt+=1; }
                        total+=1; i+=1;
                    }
                    let r = match_cnt as f64/ total.max(1) as f64;
                    if r>best.1 { best=(stride,r); }
                }
                let tail_len=b.len()-tail0;
                println!("{n}: meshcnt={} palstart={:#x} tail0={:#x} tail_len={} bestStride={} corr={:.3} | tail/stride={:.2}", cnt, palstart, tail0, tail_len, best.0, best.1, tail_len as f64/best.0 as f64);
            }
            return Ok(());
        }
        if std::env::var("DKL_TAIL").is_ok() {
            let want=std::env::var("DKL_TAIL").unwrap();
            let at: usize = std::env::var("AT").ok().and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"),16).ok()).unwrap_or(0x6000);
            let mut files=vec![]; collect_ext(&vfs, Path::new("/"), ".dkl", &mut files);
            for (n,b) in files {
                if !n.to_lowercase().contains(&want.to_lowercase()) { continue; }
                println!("{n} len={}", b.len());
                for row in 0..24 {
                    let o=at+row*16; if o+16>b.len(){break;}
                    let hex:Vec<String>=b[o..o+16].iter().map(|x| format!("{x:02x}")).collect();
                    let f:Vec<String>=(0..4).map(|i| format!("{:>11.4}", f32::from_le_bytes([b[o+i*4],b[o+i*4+1],b[o+i*4+2],b[o+i*4+3]]))).collect();
                    let asc:String=b[o..o+16].iter().map(|&x| if (32..127).contains(&x){x as char}else{'.'}).collect();
                    println!("  {o:#08x}  {}  {asc}  | {}", hex.join(" "), f.join(" "));
                }
            }
            return Ok(());
        }
        if std::env::var("DKL_MAP").is_ok() {
            let want=std::env::var("DKL_MAP").unwrap();
            let mut files=vec![];
            collect_ext(&vfs, Path::new("/"), ".dkl", &mut files);
            for (n,b) in files {
                if !n.to_lowercase().contains(&want.to_lowercase()) { continue; }
                let u=|o:usize| u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]);
                let cnt=u(0x8) as usize; let data=0xc+cnt*64;
                // count Art-xufei occurrences
                let needle=b"Art-xufei";
                let mut occ=0; let mut i=0; while i+needle.len()<=b.len(){ if &b[i..i+needle.len()]==needle {occ+=1;} i+=1; }
                println!("{n} len={} meshcount={} data_off={:#x} ArtXufei_strings={}", n.len(), cnt, data, occ);
                // material palette count at data start
                let pcnt=u(data); println!("  palette_count@data={}", pcnt);
                // walk: find offsets of all Art-xufei to compute record stride distribution
                let mut offs=vec![]; let mut i=data; while i+needle.len()<=b.len(){ if &b[i..i+needle.len()]==needle {offs.push(i);} i+=1; }
                if offs.len()>=3 {
                    let d01=offs[1]-offs[0]; let d12=offs[2]-offs[1];
                    println!("  first record strides: {} {} (first3 offs {:#x} {:#x} {:#x})", d01, d12, offs[0], offs[1], offs[2]);
                }
                // where does the last Art-xufei end? what's after?
                if let Some(&last)=offs.last() {
                    println!("  last ArtXufei @ {:#x}; tail bytes = {} ({}% of data)", last, b.len()-last, 100*(b.len()-last)/(b.len()-data));
                }
            }
            return Ok(());
        }
        if std::env::var("TEX_CMP").is_ok() {
            let want = std::env::var("TEX_CMP").unwrap();
            let mut pols = vec![];
            collect_ext(&vfs, Path::new("/"), ".pol", &mut pols);
            for (pn, pb) in pols {
                if !pn.to_lowercase().contains(&want.to_lowercase()) { continue; }
                if pb.len() < 5000 { continue; }
                let mut cur = std::io::Cursor::new(&pb);
                let pf = match fileformats::pol::read_pol(&mut cur) { Ok(x)=>x, Err(_)=>continue };
                println!("== {pn} ==");
                // POL material textures + the 16 surface floats for first few meshes
                for (mi, m) in pf.meshes.iter().take(4).enumerate() {
                    for mat in &m.material_info {
                        let names: Vec<String> = mat.texture_names.iter().map(|t| t.as_str().unwrap_or_default()).collect();
                        println!("  POL mesh{mi} mat: use_alpha={} surf16={:?} tex={:?}", mat.use_alpha, mat.unknown_68, names);
                    }
                }
                // DKL data section first material block
                let base=std::path::Path::new(&pn); let stem=base.file_stem().unwrap().to_string_lossy().to_string(); let dir=base.parent().unwrap();
                let read=|p:std::path::PathBuf| vfs.open(&p).ok().and_then(|mut f|{let mut b=vec![];std::io::Read::read_to_end(&mut f,&mut b).ok().map(|_|b)});
                if let Some(d)=read(dir.join(format!("{stem}.dkl"))).or_else(||read(dir.join(format!("{stem}.DKL")))) {
                    let cnt=u32::from_le_bytes([d[0x8],d[0x9],d[0xa],d[0xb]]) as usize;
                    let data=0xc+cnt*64;
                    // scan for ascii texture-ish strings in first 0x400 of data section
                    let mut strs=vec![]; let mut cur=String::new();
                    for &c in &d[data..(data+0x600).min(d.len())] {
                        if (32..127).contains(&c) { cur.push(c as char); } else { if cur.len()>=4 { strs.push(cur.clone()); } cur.clear(); }
                    }
                    println!("  DKL strings(head): {:?}", &strs[..strs.len().min(6)]);
                }
            }
            continue;
        }
        if std::env::var("POL_VS_DK").is_ok() {
            // For each sub-block folder, compare POL mesh count vs dkl atom count
            // and dump dkm/dkl headers.
            fn u32at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes([b[o],b[o+1],b[o+2],b[o+3]]) }
            let mut pols = vec![];
            collect_ext(&vfs, Path::new("/"), ".pol", &mut pols);
            for (pn, pb) in pols {
                // skip the small _X.POL helpers
                if pb.len() < 5000 { continue; }
                let mut cur = std::io::Cursor::new(&pb);
                let pf = match fileformats::pol::read_pol(&mut cur) { Ok(x)=>x, Err(_)=>continue };
                let total_verts: usize = pf.meshes.iter().map(|m| m.vertices.len()).sum();
                let total_mats: usize = pf.meshes.iter().map(|m| m.material_info.len()).sum();
                // sibling dkl/dkm
                let base = std::path::Path::new(&pn);
                let stem = base.file_stem().unwrap().to_string_lossy().to_string();
                let dir = base.parent().unwrap();
                let dkl_path = dir.join(format!("{stem}.dkl"));
                let dkm_path = dir.join(format!("{stem}.DKM"));
                let read=|p:&std::path::Path| -> Option<Vec<u8>> { vfs.open(p).ok().and_then(|mut f| { let mut b=vec![]; std::io::Read::read_to_end(&mut f,&mut b).ok().map(|_| b) }) };
                let dkl = read(&dkl_path).or_else(|| read(&dir.join(format!("{stem}.DKL"))));
                let dkm = read(&dkm_path).or_else(|| read(&dir.join(format!("{stem}.dkm"))));
                print!("{pn}: meshes={} verts={} mats={} geomnodes={}", pf.meshes.len(), total_verts, total_mats, pf.geom_node_descs.len());
                if let Some(d)=&dkl { print!(" | dkl: h0={:#x} h1={:#x} cnt={} len={}", u32at(d,0), u32at(d,4), u32at(d,8), d.len()); }
                if let Some(d)=&dkm { print!(" | dkm: magic={:?} v={:#x} a={} b={} sz={:#x} cnt@0x4c={} len={}", std::str::from_utf8(&d[0..4]).unwrap_or("?"), u32at(d,4), u32at(d,8), u32at(d,0xc), u32at(d,0x10), u32at(d,0x4c), d.len()); }
                println!();
            }
            return Ok(());
        }
        if std::env::var("POL_NORMALS").is_ok() {
            use fileformats::pol::PolVertexComponents as P;
            let mut pols = vec![];
            collect_ext(&vfs, Path::new("/"), ".pol", &mut pols);
            for (n, b) in pols {
                let mut cur = std::io::Cursor::new(&b);
                if let Ok(pf) = fileformats::pol::read_pol(&mut cur) {
                    for (gi, g) in pf.meshes.iter().enumerate() {
                        let vt = g.vertex_type;
                        let mut bits = vec![];
                        if vt.has(P::NORMAL){bits.push("N");}
                        if vt.has(P::UNKNOWN4){bits.push("U4");}
                        if vt.has(P::UNKNOWN8){bits.push("U8");}
                        if vt.has(P::TEXCOORD){bits.push("T1");}
                        if vt.has(P::TEXCOORD2){bits.push("T2");}
                        if vt.has(P::UNKNOWN40){bits.push("U40");}
                        if vt.has(P::UNKNOWN80){bits.push("U80");}
                        if vt.has(P::UNKNOWN100){bits.push("U100");}
                        println!("{n}#{gi} v={} flags={}", g.vertices.len(), bits.join("|"));
                    }
                }
            }
            return Ok(());
        }
        if let Ok(ext) = std::env::var("DUMP_EXT") {
            let ext = format!(".{}", ext.to_lowercase());
            let mut files = vec![];
            collect_ext(&vfs, Path::new("/"), &ext, &mut files);
            let max_rows: usize = std::env::var("ROWS").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
            for (n, b) in files.into_iter().take(3) {
                println!("\n{n} ({} bytes)", b.len());
                for row in 0..(b.len() / 16).min(max_rows) {
                    let o = row * 16;
                    let hex: Vec<String> = b[o..o + 16].iter().map(|x| format!("{x:02x}")).collect();
                    let asc: String = b[o..o + 16].iter().map(|&x| if (32..127).contains(&x) { x as char } else { '.' }).collect();
                    let f: Vec<String> = (0..4).map(|i| format!("{:>9.3}", f32::from_le_bytes([b[o+i*4],b[o+i*4+1],b[o+i*4+2],b[o+i*4+3]]))).collect();
                    println!("{o:#06x}  {}  {asc}  | {}", hex.join(" "), f.join(" "));
                }
            }
            return Ok(());
        }

        let mut scns = vec![];
        collect_scn(&vfs, Path::new("/"), &mut scns);
        for (name, buf) in scns {
            let h = match read_header(&buf) {
                Some(h) => h,
                None => continue,
            };
            file_count += 1;
            // night + skybox
            let is_night = u32::from_le_bytes([buf[0x76], buf[0x77], buf[0x78], buf[0x79]]);
            let skybox = u32::from_le_bytes([buf[0x7a], buf[0x7b], buf[0x7c], buf[0x7d]]);
            if is_night == 1 { night_count += 1; }
            skybox_ids.insert(skybox);
            let _ = name;
            for i in 0..h.role_num as usize {
                let off = h.role_offset as usize + i * ROLE_SIZE;
                if off + ROLE_SIZE <= buf.len() {
                    all_roles.push(buf[off..off + ROLE_SIZE].to_vec());
                }
            }
            for i in 0..h.node_num as usize {
                let off = h.node_offset as usize + i * NODE_SIZE;
                if off + NODE_SIZE <= buf.len() {
                    all_nodes.push(buf[off..off + NODE_SIZE].to_vec());
                }
            }
        }
    }

    println!("Parsed {file_count} .scn files | {} roles | {} nodes", all_roles.len(), all_nodes.len());
    println!("night scenes: {night_count}/{file_count} | skybox ids seen: {:?}", skybox_ids);

    analyze(&all_nodes, NODE_SIZE, "NODE", &node_known);
    analyze(&all_roles, ROLE_SIZE, "ROLE", &role_known);
    Ok(())
}
