pub mod mv3loader;
pub mod polloader;

fn read_vec(reader: &mut dyn std::io::Read, size: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; size];
    reader.read_exact(&mut buf.as_mut_slice())?;
    Ok(buf)
}
