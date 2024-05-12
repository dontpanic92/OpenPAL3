use byteorder::LittleEndian;
use xtea::XTEA;

pub struct XTea {
    xtea: XTEA,
}

impl XTea {
    pub fn new(key: &[u8]) -> Self {
        let mut state = [0xc33707d6, 0x4bdecfa9, 0xfc93a039, 0xe7d3fbc8];

        for i in 0..key.len() {
            state[i % 4] = state[i % 4] ^ (key[i] as u32);
        }

        Self {
            xtea: XTEA::new(&state),
        }
    }

    pub fn decrypt(&self, input: &[u8]) -> Vec<u8> {
        let mut output = vec![0; input.len()];
        self.xtea
            .decipher_u8slice::<LittleEndian>(input, &mut output);
        output
    }
}
