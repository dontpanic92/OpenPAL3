/*!
 TEA zpk-variant implementation
 Dervied work from https://github.com/Brekcel/xtea
 MIT Licensed
*/

use byteorder::{BigEndian, ByteOrder, ReadBytesExt, WriteBytesExt};
use std::{
    io::{Cursor, Read, Result, Write},
    num::Wrapping,
};

#[derive(Debug)]
pub struct Tea {
    key: [Wrapping<u32>; 4],
    sum: Wrapping<u32>,
}

const DELTA: Wrapping<u32> = Wrapping(0x9E3779B9);

impl Tea {
    #[inline]
    pub fn new(key: &[u8]) -> Self {
        let mut state = [0xc33707d6, 0x4bdecfa9, 0xfc93a039, 0xe7d3fbc8];

        for i in 0..key.len() {
            state[i % 4] = state[i % 4] ^ (key[i] as u32);
        }

        let sum =
            (((state[0] ^ state[1] ^ state[2] ^ state[3]) & 0xf) + 0x10) as i64 * -1640531527i64;

        let state = [
            Wrapping(state[0]),
            Wrapping(state[1]),
            Wrapping(state[2]),
            Wrapping(state[3]),
        ];

        Tea {
            key: state,
            sum: Wrapping(sum as u32),
        }
    }

    pub fn decrypt(&self, input: &[u8]) -> Vec<u8> {
        let mut output = vec![0; input.len()];
        self.decipher_u8slice::<BigEndian>(input, &mut output);
        output
    }

    #[inline]
    pub fn decipher(&self, input: &[u32; 2], output: &mut [u32; 2]) {
        let mut v0 = Wrapping(input[0]);
        let mut v1 = Wrapping(input[1]);
        let mut sum = self.sum; //DELTA * self.num_rounds;

        // for _ in 0..self.num_rounds.0 as u32 {
        while sum.0 != 0 {
            v1 -= ((v0 << 4) + self.key[2]) ^ ((v0 >> 5) + self.key[3]) ^ (sum + v0);
            v0 -= ((v1 << 4) + self.key[0]) ^ ((v1 >> 5) + self.key[1]) ^ (sum + v1);
            sum -= DELTA;
        }

        output[0] = v0.0;
        output[1] = v1.0;
    }

    #[inline]
    pub fn decipher_u8slice<B: ByteOrder>(&self, input: &[u8], output: &mut [u8]) {
        self.cipher_u8slice::<B>(input, output, false)
    }

    #[inline]
    fn cipher_u8slice<B: ByteOrder>(&self, input: &[u8], output: &mut [u8], encipher: bool) {
        assert_eq!(
            input.len(),
            output.len(),
            "The input and output slices must be of the same length."
        );
        assert_eq!(
            input.len() % 8,
            0,
            "Input and output slices must be of a length divisible by 8."
        );

        //Create cursors for the two slices, and pass it off to the stream cipher handler
        let mut input_reader = Cursor::new(input);
        let mut ouput_writer = Cursor::new(output);

        self.cipher_stream::<B, Cursor<&[u8]>, Cursor<&mut [u8]>>(
            &mut input_reader,
            &mut ouput_writer,
            encipher,
        )
        .unwrap()
    }

    #[inline]
    fn cipher_stream<B: ByteOrder, T: Read, S: Write>(
        &self,
        input: &mut T,
        output: &mut S,
        encipher: bool,
    ) -> Result<()> {
        let mut input_buf = [0 as u32; 2];
        let mut output_buf = [0 as u32; 2];

        loop {
            //An error parsing the first value means we should stop parsing, not fail
            input_buf[0] = match input.read_u32::<B>() {
                Ok(val) => val,
                Err(_) => break,
            };
            input_buf[1] = input.read_u32::<B>()?;

            if encipher {
                panic!("TEA encipher not supported");
            } else {
                self.decipher(&input_buf, &mut output_buf);
            }

            output.write_u32::<B>(output_buf[0])?;
            output.write_u32::<B>(output_buf[1])?;
        }
        Ok(())
    }
}
