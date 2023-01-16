use std::{io::Cursor, path::PathBuf};

use fileformats::pol::read_pol;
use shared::exporters::{obj_exporter::export_to_file, pol_obj_exporter::export_pol_to_obj};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("使用方法: {} A.pol output.obj", args[0]);
        return;
    }

    let data = std::fs::read(&args[1]).unwrap();
    let mut reader = Cursor::new(data);
    let pol = read_pol(&mut reader).unwrap();

    let name = PathBuf::from(&args[1])
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let obj = export_pol_to_obj(Some(&pol), &name);
    if let Some(obj) = obj {
        if let Ok(()) = export_to_file(&obj.0, &obj.1, &args[2]) {
            println!("导出成功");
            return;
        }

        println!("导出失败：无法导出 obj 文件");
        return;
    }

    println!("导出失败：转换为 obj 格式");
}
