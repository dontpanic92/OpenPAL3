use crate::fs::cpk::CpkArchive;

pub fn load_smp(data: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let mut cpk = CpkArchive::load(Box::new(std::io::Cursor::new(data)))?;
    let name = cpk.file_names[0].clone();
    let mut content = cpk.open_str(&name)?.content();
    let size = content.len() & 0xFFFFFFFC;
    content.resize(size, 0);

    let decrypted = xxtea::decrypt_raw(
        &content,
        "Vampire.C.J at Softstar Technology (ShangHai) Co., Ltd",
    );

    Ok(decrypted)
}
