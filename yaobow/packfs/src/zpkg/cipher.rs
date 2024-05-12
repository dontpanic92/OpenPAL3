use tomcrypt_sys::{ltc_cipher_descriptor, Symmetric_key};

pub struct Cipher {
    key_len: usize,
    rem_xor: u8,
    block_size: u32,
    cipher: ltc_cipher_descriptor,
}

impl Cipher {
    pub fn new(
        key_len: usize,
        rem_xor: u8,
        block_size: u32,
        cipher: ltc_cipher_descriptor,
    ) -> Self {
        Self {
            key_len,
            rem_xor,
            block_size,
            cipher,
        }
    }

    pub fn setup(&self, key: &[u8]) -> CipherInstance {
        let mut key = Self::initialize_key(&key, self.key_len);
        let skey = unsafe {
            let mut skey: Symmetric_key = std::mem::zeroed();

            if self.cipher.ID == 1 {
                // xtea
                super::swap_endian(&mut key);
            }

            if let Some(setup) = self.cipher.setup {
                setup(key.as_ptr(), self.key_len as i32, 0, &mut skey);
            }
            skey
        };

        CipherInstance {
            rem_xor: self.rem_xor,
            block_size: self.block_size,
            cipher: self.cipher,
            skey,
        }
    }

    fn initialize_key(key: &[u8], output_len: usize) -> Vec<u8> {
        if output_len < key.len() {
            let mut output = key[0..output_len].to_vec();
            for i in output_len..key.len() {
                output[i % output_len] ^= key[i];
            }

            output
        } else {
            let mut output = key.to_vec();
            let mut x = key[0] as i32;
            for _ in key.len()..output_len {
                x = ((x * -0x3e39b193 + 0x4473) >> 0x10) & 0x7fff;
                output.push(x as u8);
            }

            output
        }
    }
}

pub struct CipherInstance {
    rem_xor: u8,
    block_size: u32,
    cipher: ltc_cipher_descriptor,
    skey: Symmetric_key,
}

impl CipherInstance {
    pub fn decrypt(&mut self, buffer: &[u8]) -> Vec<u8> {
        match (self.cipher.ID, self.block_size) {
            (1, _) => {
                // xtea

                let mut buffer = buffer.to_vec();
                super::swap_endian(&mut buffer);

                let mut ret = decrypt2(&buffer, &self.cipher, self.rem_xor, &mut self.skey);
                super::swap_endian(&mut ret);
                ret
            }
            (_, 2) => decrypt2(buffer, &self.cipher, self.rem_xor, &mut self.skey),
            (_, 4) => decrypt4(buffer, &self.cipher, self.rem_xor, &mut self.skey),
            _ => panic!("Unsupported decrypt step length"),
        }
    }
}

impl Drop for CipherInstance {
    fn drop(&mut self) {
        if let Some(done) = self.cipher.done {
            unsafe { done(&mut self.skey) };
        }
    }
}

fn decrypt4(
    buffer: &[u8],
    c: &ltc_cipher_descriptor,
    rem_xor: u8,
    skey: &mut Symmetric_key,
) -> Vec<u8> {
    let mut output = vec![];
    let step = buffer.len() / 16;

    for i in 0..step {
        let input_buf = &buffer[i * 16..(i + 1) * 16];
        let mut output_buf = vec![0u8; 16];

        if let Some(decrypt) = c.ecb_decrypt {
            unsafe { decrypt(input_buf.as_ptr(), output_buf.as_mut_ptr(), skey) };
        }

        output.append(&mut output_buf);
    }

    for i in step * 16..buffer.len() {
        output.push((buffer.len() - i) as u8 ^ buffer[i] ^ rem_xor);
    }

    output
}

fn decrypt2(
    buffer: &[u8],
    c: &ltc_cipher_descriptor,
    rem_xor: u8,
    skey: &mut Symmetric_key,
) -> Vec<u8> {
    let mut output = vec![];
    let step = buffer.len() / 8;

    for i in 0..step {
        let input_buf = &buffer[i * 8..(i + 1) * 8];
        let mut output_buf = vec![0u8; 8];

        if let Some(decrypt) = c.ecb_decrypt {
            unsafe { decrypt(input_buf.as_ptr(), output_buf.as_mut_ptr(), skey) };
        }

        output.append(&mut output_buf);
    }

    for i in step * 8..buffer.len() {
        output.push((buffer.len() - i) as u8 ^ buffer[i] ^ rem_xor);
    }

    output
}
