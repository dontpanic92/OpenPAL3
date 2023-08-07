use binrw::binrw;

#[binrw]
#[brw(little)]
#[derive(Clone)]
pub struct SizedString {
    #[bw(calc(string.len() as u32))]
    size: u32,

    #[br(count = size)]
    string: Vec<u8>,
}

impl SizedString {
    pub fn data(&self) -> &[u8] {
        &self.string
    }
}

impl<T: AsRef<str>> From<T> for SizedString {
    fn from(value: T) -> Self {
        Self {
            string: value.as_ref().as_bytes().to_vec(),
        }
    }
}

impl From<SizedString> for String {
    fn from(value: SizedString) -> Self {
        String::from_utf8_lossy(&value.string).to_string()
    }
}

impl std::fmt::Debug for SizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SizedString(\"{}\")",
            String::from_utf8_lossy(&self.string)
        )
    }
}

impl PartialEq<&str> for SizedString {
    fn eq(&self, other: &&str) -> bool {
        String::from_utf8_lossy(&self.string) == *other
    }
}
