// Spans and extensions are packed in a very particular manner: 8
// elements are packed into a stride of as many bytes are necessary to
// contain them (or, one byte for each bit in an element). Within each
// stride, elements are generally packed from first to last; however,
// within each byte elements are packed from right to left, rather
// than left to right.
//
// For example, 3-bit elements would be packed as
//
//   22111000 54443332 77766655

// ATTENTION READERS:
//
//   DO NOT COPY THESE PACKING FUNCTIONS UNLESS YOU NEED THIS VERY
//   SPECIFIC LAYOUT. THIS IS NOT THE BEST WAY TO PACK BITFIELDS.
//   THESE *ONLY* EXIST FOR COMPATIBILITY REASONS.
//
//   YOU PROBABLY WANT TO PACK YOUR BITFIELDS FROM LEFT TO RIGHT.
//   THESE DO NOT DO THAT.

use std::collections::VecDeque;

#[inline]
pub(in super) fn pack3_size(count: usize) -> usize {
    //(count / 8 + 1) * 3
    (count * 3 + 7) / 8
}

/// Pack three-bit elements in the same fashion as fpcalc
#[inline]
pub(in super) fn pack3(elems: Vec<u8>, out: &mut Vec<u8>) {
    let mut strides = elems.chunks_exact(8);
    while let Some(stride) = strides.next() {
        out.push((stride[0] & 0b111) | ((stride[1] & 0b111) << 3) | (stride[2] << 6));

        out.push(
            ((stride[2] & 0b100) >> 2)
                | ((stride[3] & 0b111) << 1)
                | ((stride[4] & 0b111) << 4)
                | (stride[5] << 7),
        );

        out.push(
            ((stride[5] & 0b110) >> 1) | ((stride[6] & 0b111) << 2) | ((stride[7] & 0b111) << 5),
        );
    }

    let tail = strides.remainder();
    if !tail.is_empty() {
        if let Some(el5) = tail.get(6) {
            out.push((tail[0] & 0b111) | ((tail[1] & 0b111) << 3) | (tail[2] << 6));

            out.push(
                ((tail[2] & 0b100) >> 2)
                    | ((tail[3] & 0b111) << 1)
                    | ((tail[4] & 0b111) << 4)
                    | (tail[5] << 7),
            );

            out.push(((tail[5] & 0b110) >> 1) | ((el5 & 0b111) << 2));
        } else if let Some(el5) = tail.get(5) {
            out.push((tail[0] & 0b111) | ((tail[1] & 0b111) << 3) | (tail[2] << 6));

            out.push(
                ((tail[2] & 0b100) >> 2)
                    | ((tail[3] & 0b111) << 1)
                    | ((tail[4] & 0b111) << 4)
                    | (el5 << 7),
            );

            out.push((el5 & 0b110) >> 1);
        } else if let Some(el4) = tail.get(4) {
            out.push((tail[0] & 0b111) | ((tail[1] & 0b111) << 3) | (tail[2] << 6));
            out.push(((tail[2] & 0b100) >> 2) | ((tail[3] & 0b111) << 1) | ((el4 & 0b111) << 4));
        } else if let Some(el3) = tail.get(3) {
            out.push((tail[0] & 0b111) | ((tail[1] & 0b111) << 3) | (tail[2] << 6));
            out.push(((tail[2] & 0b100) >> 2) | ((el3 & 0b111) << 1));
        } else if let Some(el2) = tail.get(2) {
            out.push((tail[0] & 0b111) | ((tail[1] & 0b111) << 3) | (el2 << 6));
            out.push((el2 & 0b100) >> 2);
        } else if let Some(el1) = tail.get(1) {
            out.push((tail[0] & 0b111) | ((el1 & 0b111) << 3));
        } else {
            out.push(tail[0] & 0b111)
        }
    }
}

#[inline]
pub(in super) fn unpack3(elems: &[u8]) -> VecDeque<u8> {
    let mut out: VecDeque<u8> = VecDeque::with_capacity(elems.len() * 3 - elems.len());
    let mut strides = elems.chunks_exact(3);

    while let Some(stride) = strides.next() {
        out.push_back(stride[0] & 0b111);
        out.push_back((stride[0] >> 3) & 0b111);
        out.push_back((stride[0] >> 6) | ((stride[1] & 0b001) << 2));
        out.push_back((stride[1] >> 1) & 0b111);
        out.push_back((stride[1] >> 4) & 0b111);
        out.push_back((stride[1] >> 7) | ((stride[2] & 0b011) << 1));
        out.push_back((stride[2] >> 2) & 0b111);
        out.push_back(stride[2] >> 5);
    }

    let stride = strides.remainder();

    if let Some(stride1) = stride.get(1) {
        out.push_back(stride[0] & 0b111);
        out.push_back((stride[0] >> 3) & 0b111);
        out.push_back((stride[0] >> 6) | ((stride1 & 0b001) << 2));
        out.push_back((stride1 >> 1) & 0b111);
        out.push_back((stride1 >> 4) & 0b111);
    } else if let Some(stride0) = stride.get(0) {
        out.push_back(stride0 & 0b111);
        out.push_back((stride0 >> 3) & 0b111);
    }


    out
}

pub(in super) fn pack5_size(count: usize) -> usize {
    (count / 8 + 1) * 5
}

// similarly, 5-bit extensions are packed into 40-bit groups,
#[inline]
pub(in super) fn pack5(elems: Vec<u8>, out: &mut Vec<u8>) {
    let mut strides = elems.chunks_exact(8);
    while let Some(chunk) = strides.next() {
        // 11100000
        out.push((chunk[0] & 0b11111) | (chunk[1] << 5));
        // 32222211
        out.push(((chunk[1] & 0b11000) >> 3) | ((chunk[2] & 0b11111) << 2) | (chunk[3] << 7));
        // 44443333
        out.push(((chunk[3] & 0b11110) >> 1) | (chunk[4] << 4));
        // 66555554
        out.push(((chunk[4] & 0b10000) >> 4) | ((chunk[5] & 0b11111) << 1) | (chunk[6] << 6));
        // 77777666
        out.push(((chunk[6] & 0b11100) >> 2) | ((chunk[7] & 0b11111) << 3));
    }

    let tail = strides.remainder();
    if !tail.is_empty() {
        if let Some(el6) = tail.get(6) {
            out.push((tail[0] & 0b11111) | (tail[1] << 5));
            out.push(((tail[1] & 0b11000) >> 3) | ((tail[2] & 0b11111) << 2) | (tail[3] << 7));
            out.push(((tail[3] & 0b11110) >> 1) | (tail[4] << 4));
            out.push(((tail[4] & 0b10000) >> 4) | ((tail[5] & 0b11111) << 1) | (el6 << 6));
            out.push((el6 & 0b11100) >> 2);
        } else if let Some(el5) = tail.get(5) {
            out.push((tail[0] & 0b11111) | (tail[1] << 5));
            out.push(((tail[1] & 0b11000) >> 3) | ((tail[2] & 0b11111) << 2) | (tail[3] << 7));
            out.push(((tail[3] & 0b11110) >> 1) | (tail[4] << 4));
            out.push(((tail[4] & 0b10000) >> 4) | ((el5 & 0b11111) << 1));
        } else if let Some(el4) = tail.get(4) {
            out.push((tail[0] & 0b11111) | (tail[1] << 5));
            out.push(((tail[1] & 0b11000) >> 3) | ((tail[2] & 0b11111) << 2) | (tail[3] << 7));
            out.push(((tail[3] & 0b11110) >> 1) | (el4 << 4));
            out.push((el4 & 0b10000) >> 4);
        } else if let Some(el3) = tail.get(3) {
            out.push((tail[0] & 0b11111) | (tail[1] << 5));
            out.push(((tail[1] & 0b11000) >> 3) | ((tail[2] & 0b11111) << 2) | (el3 << 7));
            out.push((el3 & 0b11110) >> 1);
        } else if let Some(el2) = tail.get(2) {
            out.push((tail[0] & 0b11111) | (tail[1] << 5));
            out.push(((tail[1] & 0b11000) >> 3) | ((el2 & 0b11111) << 2))
        } else if let Some(el1) = tail.get(1) {
            out.push((tail[0] & 0b11111) | (el1 << 5));
            out.push((el1 & 0b11000) >> 3)
        } else {
            out.push(tail[0] & 0b11111);
        }
    }
}

#[inline]
pub(in super) fn unpack5(elems: &[u8]) -> VecDeque<u8> {
    let mut out: VecDeque<u8> = VecDeque::with_capacity(elems.len() * 5 - elems.len());
    let mut strides = elems.chunks_exact(5);

    while let Some(stride) = strides.next() {
        out.push_back(stride[0] & 0b11111);
        out.push_back((stride[0] >> 5) | ((stride[1] & 0b11) << 3));
        out.push_back((stride[1] >> 2) & 0b11111);
        out.push_back((stride[1] >> 7) | ((stride[2] & 0b1111) << 1));
        out.push_back((stride[2] >> 4) | ((stride[3] & 0b1) << 4));
        out.push_back((stride[3] >> 1) & 0b11111);
        out.push_back((stride[3] >> 6) | ((stride[4] & 0b111) << 2));
        out.push_back(stride[4] >> 3);
    }

    let stride = strides.remainder();
    if !stride.is_empty() {
        out.push_back(stride[0] & 0b11111);

        if let Some(stride1) = stride.get(1) {
            out.push_back((stride[0] >> 5) | ((stride1 & 0b11) << 3));
            out.push_back((stride1 >> 2) & 0b11111);
        }

        if let Some(stride2) = stride.get(2) {
            out.push_back((stride[1] >> 7) | ((stride2 & 0b1111) << 1));
        }

        if let Some(stride3) = stride.get(3) {
            out.push_back((stride[2] >> 4) | ((stride3 & 0b1) << 4));
            out.push_back((stride3 >> 1) & 0b11111);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! validates_pack {
        ($fn:ident, $input:expr => $output:expr) => {
            let input = $input;
            let expected = $output;
            let mut dest: Vec<u8> = Vec::with_capacity(expected.len());

            $fn(input, &mut dest);

            assert_eq!(dest, expected, "Pack output did not match expected output");
        };
    }

    macro_rules! validates_unpack {
        ($fn:ident, $input:expr => $expected:expr) => {
            let input = $input;
            let output = $fn(&input);

            assert_eq!(output, $expected, "Unpack output did not match expected output");
        };
    }

    #[test]
    fn pack3_output() {
        validates_pack!(pack3, vec![0u8, 1, 2, 3, 4, 5, 6, 7, 6] => [0b10_001_000u8, 0b1_100_011_0, 0b111_110_10, 0b00_000_110]);
        validates_pack!(pack3, vec![0u8, 1, 2, 3, 4, 5, 6, 7] => [0b10_001_000u8, 0b1_100_011_0, 0b111_110_10]);
        validates_pack!(pack3, vec![0u8, 1, 2, 3, 4, 5, 6] => [0b10_001_000u8, 0b1_100_011_0, 0b000_110_10]);
        validates_pack!(pack3, vec![0u8, 1, 2, 3, 4, 5] => [0b10_001_000u8, 0b1_100_011_0, 0b000_000_10]);
        validates_pack!(pack3, vec![0u8, 1, 2, 3, 4] => [0b10_001_000u8, 0b0_100_011_0]);
        validates_pack!(pack3, vec![0u8, 1, 2, 3] => [0b10_001_000u8, 0b0_000_011_0]);
        validates_pack!(pack3, vec![0u8, 1, 2] => [0b10_001_000u8, 0b0_000_000_0]);
        validates_pack!(pack3, vec![0u8, 1] => [0b00_001_000u8]);
        validates_pack!(pack3, vec![1u8] => [0b00_000_001u8]);
        validates_pack!(pack3, vec![] => []);
    }

    #[test]
    fn unpack3_output() {
        validates_unpack!(unpack3, [0b10_001_000u8, 0b1_100_011_0, 0b111_110_10, 0b00_000_110] => vec![0u8, 1, 2, 3, 4, 5, 6, 7, 6, 0]);
        validates_unpack!(unpack3, [0b10_001_000u8, 0b1_100_011_0, 0b111_110_10] => vec![0u8, 1, 2, 3, 4, 5, 6, 7]);
        validates_unpack!(unpack3, [0b10_001_000u8, 0b0_100_011_0] => vec![0u8, 1, 2, 3, 4]);
        validates_unpack!(unpack3, [0b00_001_000u8] => vec![0u8, 1]);
        validates_unpack!(unpack3, [] => vec![]);
    }

    #[test]
    fn pack3_roundtrip() {
        let input = [4u8, 2, 0, 2, 2, 5, 4, 5, 3, 0, 0, 0, 2, 1, 1, 2, 1, 6, 4, 0, 1, 3, 2, 6];

        let mut packed: Vec<u8> = Vec::new();
        let mut pack_input: Vec<u8> = Vec::new();
        pack_input.resize(input.len(), 0);
        pack_input.copy_from_slice(&input);
        pack3(pack_input, &mut packed);

        let unpacked = unpack3(&packed);

        assert_eq!(unpacked, input, "unpack3(pack3(input)) != input");
    }

    #[test]
    fn pack5_output() {
        // 0001 00010 00011 00100 00101 00110 00111 01000
        validates_pack!(pack5, vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8] => [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b10_00101_0, 0b00111_001, 0b000_01000]);
        validates_pack!(pack5, vec![0u8, 1, 2, 3, 4, 5, 6, 7] => [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b10_00101_0, 0b00111_001]);
        validates_pack!(pack5, vec![0u8, 1, 2, 3, 4, 5, 6] => [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b10_00101_0, 0b00000_001]);
        validates_pack!(pack5, vec![0u8, 1, 2, 3, 4, 5] => [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b00_00101_0]);
        validates_pack!(pack5, vec![0u8, 1, 2, 3, 4] => [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b00_00000_0]);
        validates_pack!(pack5, vec![0u8, 1, 2, 3] => [0b001_00000, 0b1_00010_00, 0b0000_0001]);
        validates_pack!(pack5, vec![0u8, 1, 2] => [0b001_00000, 0b0_00010_00]);
        validates_pack!(pack5, vec![0u8, 1] => [0b001_00000, 0b0_00000_00]);
        validates_pack!(pack5, vec![15u8] => [0b000_01111]);
        validates_pack!(pack5, vec![] => []);
    }

    #[test]
    fn unpack5_output() {
        validates_unpack!(unpack5, [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b10_00101_0, 0b00111_001, 0b000_01000] => vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
        validates_unpack!(unpack5, [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b10_00101_0, 0b00111_001] => vec![0u8, 1, 2, 3, 4, 5, 6, 7]);
        validates_unpack!(unpack5, [0b001_00000, 0b1_00010_00, 0b0100_0001, 0b00_00101_0] => vec![0u8, 1, 2, 3, 4, 5]);
        validates_unpack!(unpack5, [0b001_00000, 0b1_00010_00, 0b0000_0001] => vec![0u8, 1, 2, 3]);
        validates_unpack!(unpack5, [0b001_00000, 0b0_00010_00] => vec![0u8, 1, 2]);
        validates_unpack!(unpack5, [0b000_01111] => vec![15u8]);
        validates_unpack!(unpack5, [] => vec![]);
    }

    #[test]
    fn pack5_roundtrip() {
        let input = [15u8, 12, 20, 31, 10, 4, 2, 5, 6, 8, 12, 20, 0, 0, 13, 12];

        let mut packed: Vec<u8> = Vec::new();
        let mut pack_input: Vec<u8> = Vec::new();
        pack_input.resize(input.len(), 0);
        pack_input.copy_from_slice(&input);
        pack5(pack_input, &mut packed);

        let unpacked = unpack5(&packed);

        assert_eq!(unpacked, input, "unpack5(pack5(input)) != input");
    }
}
