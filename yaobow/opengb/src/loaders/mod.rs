pub mod cvd_loader;
pub mod nav_loader;
pub mod pol;
pub mod sce_loader;
pub mod scn_loader;

fn calc_vertex_size(t: i32) -> usize {
    if t < 0 {
        return (t & 0x7FFFFFFF) as usize;
    }

    let mut size = 0;

    if t & 1 != 0 {
        size += 12;
    }

    if t & 2 != 0 {
        size += 12;
    }

    if t & 4 != 0 {
        size += 4;
    }

    if t & 8 != 0 {
        size += 4;
    }

    if t & 0x10 != 0 {
        size += 8;
    }

    if t & 0x20 != 0 {
        size += 8;
    }

    if t & 0x40 != 0 {
        size += 8;
    }

    if t & 0x80 != 0 {
        size += 8;
    }

    if t & 0x100 != 0 {
        size += 16;
    }

    return size;
}
