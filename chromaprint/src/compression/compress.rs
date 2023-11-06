use super::pack::{pack3, pack5, pack3_size, pack5_size};

// this implementation is a pretty close copy of the original C++
// compressor, particularly with respect to some of the assumptions
// around allocation, as they were likely determined from observation
// of typical data.
/// Perform delta compression on sub-fingerprints, producing a
/// compressed representation suitable for use with AcoustID.
pub fn compress_fingerprint(fp: &[u32], fp_algo: u8) -> Vec<u8> {
    // 3-bit span measurements for the delta encoding
    let mut spans: Vec<u8> = Vec::with_capacity(fp.len()); // TODO: may be under-reserved, assumes 1:4.

    // remaining 5-bit span extensions for long spans
    let mut span_extensions: Vec<u8> = Vec::with_capacity(fp.len() / 10); // also worth investigating capacity

    let mut last_sub_fp = 0u32;
    let mut cursor = fp.iter();
    while let Some(sub_fp) = cursor.next() {
        let mut bit_index = 1u8;
        let mut last_bit_index = 0u8;

        // perform the pre-compression XOR between the sub-fp and its
        // predecessor.
        let mut precompressed_fp = sub_fp ^ last_sub_fp;
        last_sub_fp = *sub_fp;

        // walk through all set (1) bits, computing the span between them.
        while precompressed_fp != 0 {
            if (precompressed_fp & 1) != 0 {
                let span = bit_index - last_bit_index;

                // if the distance between set bits can't be expressed in
                // three bits, use one of the extension cells to represent the
                // remainder
                if span >= 0b111 {
                    spans.push(0b111);
                    span_extensions.push(span - 0b111);
                } else {
                    spans.push(span);
                }

                // mark this position as the last set bit for the next span
                // measurement.
                last_bit_index = bit_index;
            }

            precompressed_fp >>= 1;
            bit_index += 1;
        }

        // mark the end of the sub-fp with a zero
        spans.push(0);
    }

    let spans_size = pack3_size(spans.len());
    let exts_size = pack5_size(span_extensions.len());

    let mut output: Vec<u8> = Vec::with_capacity(4 + spans_size + exts_size);

    output.push(fp_algo);
    output.push(((fp.len() >> 16) & 0xFF) as u8);
    output.push(((fp.len() >> 8) & 0xFF) as u8);
    output.push((fp.len() & 0xFF) as u8);

    pack3(spans, &mut output);
    pack5(span_extensions, &mut output);

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // The official test suite

    /// Verify that the encoder emits a span.
    #[test]
    fn one_item_one_bit() {
        let fingerprint = [1u32];

        let compressed = compress_fingerprint(&fingerprint, 0);

        assert_eq!(compressed, vec![0, 0, 0, 1, 0b00000001]);
    }

    /// Verify that the packer writes two bytes of data, as the
    /// encoder will produce three spans (1-1-1), requiring 9 bits of
    /// storage.
    #[test]
    fn one_item_three_bits() {
        let fingerprint = [0b111u32];

        let compressed = compress_fingerprint(&fingerprint, 0);

        assert_eq!(compressed, vec![0, 0, 0, 1, 0b01001001, 0]);
    }

    /// Verify that the encoder writes both a span and an extension
    /// when it encounters a span of >=7 bits. In this case, for the
    /// number 64, the first bit set is #7, and as such we should
    /// expect that a span of 7 (0b111) is emitted, as well as an
    /// empty extension.
    #[test]
    fn one_item_one_bit_except() {
        let fingerprint = [0b1000000];

        let compressed = compress_fingerprint(&fingerprint, 0);

        // element 4 should be the span, 5 the extension
        assert_eq!(compressed, vec![0, 0, 0, 1, 0b00000111, 0]);
    }

    /// As above, but verify that the encoder correctly computes the
    /// span extension. For 256, the first set bit is #9. As such, we
    /// expect a span of 7 (0b111) and an extension of 2 (0b10).
    #[test]
    fn one_item_one_bit_except_2() {
        let fingerprint = [0b100000000u32];

        let compressed = compress_fingerprint(&fingerprint, 0);

        assert_eq!(compressed, vec![0, 0, 0, 1, 0b00000111, 0b00000010]);
    }

    /// As in one_item_one_bit, we expect that the number 1 is encoded
    /// as a single span of 1. We expect that for a 1 and 0, two spans
    /// of 1 will be emitted (1-0). Two spans are emitted because
    /// successive sub-fingerprints are XORed. Because two
    /// subfingerprints are present, there will be a marker span of 0.
    /// The span sequence (1-0-1) requires 9 bits, so we expect two
    /// bytes to be emitted by the packer.
    #[test]
    fn two_items() {
        let fingerprint = [1u32, 0];

        let compressed = compress_fingerprint(&fingerprint, 0);

        //                                   ˯ Span count
        //                                   |    ˯ Second span of 1, low bit
        //                                   |    |˯˯˯ Marker span (0)
        //                                   |    ||||˯˯˯ First span of 1
        assert_eq!(compressed, vec![0, 0, 0, 2, 0b1000001, 0]);
    }

    /// As above, but for two successive identical subfingerprints. In
    /// this case, only one span of 1 should be emitted, and two spans
    /// of zero - again, because successive fingerprints are XORed.
    #[test]
    fn two_items_no_change() {
        let fingerprint = [1u32; 2];

        let compressed = compress_fingerprint(&fingerprint, 0);

        assert_eq!(compressed, vec![0, 0, 0, 2, 0b00000001, 0]);
    }

    /// More of an explanatory test than anything - related to
    /// compare_to_reference but at a smaller scale (only as many
    /// subfingerprints as needed to create aligned output).
    /// Has comments outlining the structure of data.
    #[rustfmt::skip]
    #[test]
    fn large_sub_fp_multiple() {
        let fingerprint = [
            // 2083237405
            0b0111_1100_0010_1011_1010_1110_0001_1101u32,
            // 2083321372
            0b0111_1100_0010_1100_1111_0110_0001_1100u32,
            // 2034029340
            0b0111_1001_0011_1100_1101_0011_0001_1100u32,
            // 2036092988
            0b0111_1001_0101_1100_0101_0000_0011_1100u32,
        ];

        let compressed = compress_fingerprint(&fingerprint, 0);

        assert_eq!(
            compressed,
            vec![
                // compression header: version 0, 2 subfingerprints
                0, 0, 0, 4,
                // subfingerprint 1
                0b01_010_001, 0b1_101_001_0, 0b010_001_00, 0b01_001_010, 0b1_010_010_0, 0b001_001_10, 0b00_001_001,
                // subfingerprint 2, plus high bit of subfingerprint 1 marker
                //          ˯ rest of subfingerprint 1 marker
                0b1_111_001_0, 0b010_010_00, 0b00_001_001,
                // subfingerprint 3
                //          ˯ rest of subfingerprint 2 marker
                0b1_010_111_0, 0b100_111_01,
                // subfingerprint 4
                //   ˯ end of subfingerprint 3
                0b10_000_010, 0b0_001_011_1, 0b001_110_11, 0b00_000_000,
                // extensions
                0b010_00100, 0b0_00000_00
            ]
        );
    }

    #[test]
    fn compare_to_reference() {
        // a raw fingerprint as computed by fpcalc,
        let raw: Vec<u32> = vec![
            2083237405, 2083321372, 2034029340, 2036092988, 2076979244, 2060197924, 2055479332,
            2055544868, 2072387685, 2038564198, 2021854822, 2021657158, 952122054, 406858438,
            423651974, 453111686, 436643718, 436643206, 436708742, 436583830, 973388215, 973380055,
            994556229, 726051653, 709279493, 709278981, 709270549, 709139495, 704691239, 704691239,
            705002534, 688274470, 671495974, 671483430, 675677799, 679806567, 679743079, 679751237,
            679751245, 679750221, 679750221, 952400717, 952432989, 960887135, 1002831199,
            986054015, 986054015, 986183007, 986183007, 1002960351, 1002796543, 969375215,
            415662061, 407363565, 407363516, 407232388, 407224197, 407224215, 407158775, 407224310,
            457554806, 449247510, 445577478, 445545734, 445611062, 445463591, 449658159, 453844269,
            420417325, 403507020, 403441220, 402917189, 407127925, 415528311, 415624546, 415755634,
            415738354, 411281874, 428063186, 998480338, 981645762, 977451458, 704821714,
            1778621906, 1778425078, 1778433127, 1782627429, 1782692965, 1778531428, 721566820,
            671489380, 671485540, 675679844, 679937637, 679741045, 679751237, 679751245, 948186701,
            944008781, 944045773, 944111565, 944111581, 961019869, 994443135, 977655167, 977753471,
            977425758, 985888094, 465927294, 465861742, 432373103, 407337309, 407306061, 407297869,
            407369413, 407237621, 415560693, 138736101, 138736103, 138932727, 138868727, 138936306,
            147192819, 142932979, 1216872403, 1216806849, 1216796609, 1216792257, 1306445377,
            1165967985, 1164919665, 1097814897, 1106112881, 1106112627, 1097725043, 1097848147,
            1097848147, 1097840593, 1097709265, 1097709297, 1097725681, 1097855729, 1097855731,
            3245200371, 3245201266, 3278731122, 3278863730, 1122975090, 1190083698, 1189682290,
            1319705714, 1319711826, 1252601905, 174927920, 171782176, 169685344, 168046944,
            168063840, 168084832, 453041507, 423668838, 423667826, 960735346, 1002678482,
            998488274, 998480338, 981645762, 985841602, 709017554, 704815090, 1778617571,
            1782881509, 709147877, 709143780, 705035428, 721829028, 671488420, 671485604,
            671485604, 684068516, 679743213, 679751389, 679751181, 679751181, 948186893, 432291613,
            407140159, 407172895, 407238431, 407213343, 415604061, 415603069, 415603053, 986167645,
            985840845, 977450189, 973246669, 973443261, 973123759, 989967279, 939717294, 939685630,
            944193278, 943995518, 952384302, 965032238, 981940270, 981933086, 445002765, 445067276,
            440668444, 453251884, 419763052, 419895140, 402933604, 402673015, 406899062, 415287510,
            432253142, 432187586, 465742290, 461613522, 461486546, 998489554, 998424515, 994229955,
            1778556625, 1778617697, 1778424932, 1782627556, 1782623460, 1778776292, 1795635428,
            1745238516, 671489764, 671483620, 684065508, 679740005, 679744053, 679748357,
            948183813, 411313927, 139208455, 139261719, 139261767, 147832263, 147766727, 147758535,
            147230149, 164007109, 189177029, 172397763, 168214723, 185057490, 185057778, 155612130,
            155591394, 164045554, 163854066, 159676019, 159678305, 159776544, 1267071793,
            1254488369, 1246099729, 1246223633, 1321719063, 1321554230, 1317368614, 113599271,
            1179087463, 1443119973, 1376569189, 1376570725, 1397541237, 1388890493, 1388943871,
            1380686319, 1380645358, 1389099230, 1388936414, 1384737887, 1368012927, 1355430253,
            1347033965, 1078606461, 1078550108, 1078484556, 1086904157, 1103688031, 1137242623,
            1204417006, 1162605038, 1296781822, 1288394206, 1288666590, 1288645086, 138820095,
            134626765, 134640973, 134669645, 134407501, 134391117, 155297109, 155297143, 188851319,
            172209239, 172208214, 172150998, 180568150, 197202015, 163721311, 138548597, 138550885,
            134684261, 138763869, 138722893, 163624525, 197168989, 448896373, 440511605, 440511541,
            436319285, 973312053, 973312053, 973254709, 973254773, 704811105, 704807009, 709086305,
            727042417, 693487957, 678812116, 678812132, 677763556, 677726628, 677710228, 694499716,
            703937461, 685058981, 684993191, 684010134, 684141126, 680991254, 680856118, 682900022,
            750004758, 750004758, 748956166, 749472262, 627964279, 627964279, 627964279, 627964279,
            627964279, 627964279, 627964279, 627964279, 627964279, 627964279,
        ];

        // and the corresponding encoded fingerprint:
        let known_compressed: Vec<u8> = vec![
            0x01, 0x00, 0x01, 0x68, 0x51, 0xd2, 0x44, 0x4a, 0xa4, 0x26, 0x09, 0xf2, 0x48, 0x09,
            0xae, 0x9d, 0x82, 0x17, 0x3b, 0xe8, 0xe3, 0x09, 0xfc, 0xf1, 0xe1, 0x88, 0x9f, 0x23,
            0xb9, 0xd6, 0xe3, 0x51, 0x0f, 0xd7, 0xc1, 0x9b, 0x1f, 0xbf, 0xf1, 0x1f, 0x57, 0xe3,
            0xe1, 0x53, 0x62, 0x1c, 0x47, 0x9f, 0x22, 0xad, 0x0f, 0xe7, 0xd0, 0xe6, 0xa5, 0xc2,
            0xc7, 0xe3, 0xd4, 0x71, 0x94, 0x85, 0x96, 0xe3, 0x25, 0x90, 0x27, 0xc2, 0x95, 0x07,
            0x8f, 0x8e, 0x33, 0x88, 0x8f, 0xa7, 0xc1, 0x0b, 0xf1, 0x20, 0x0e, 0x9c, 0xba, 0xd1,
            0x1a, 0xfa, 0x83, 0x5f, 0xf0, 0x01, 0xdb, 0xc0, 0x0f, 0x5f, 0xe8, 0x7d, 0xe8, 0x7f,
            0x71, 0xe5, 0x08, 0x05, 0x26, 0x47, 0x1e, 0x68, 0x70, 0x8e, 0x1c, 0xdf, 0x03, 0x67,
            0x55, 0x1c, 0xf4, 0xc3, 0x8b, 0x66, 0x47, 0xf8, 0x81, 0x3d, 0xf4, 0x27, 0xf8, 0x7a,
            0xa4, 0xa9, 0x0f, 0xf6, 0xc8, 0x8f, 0xe6, 0x87, 0xbe, 0x1c, 0x91, 0x9e, 0x05, 0x3d,
            0x3e, 0xc2, 0x27, 0x7e, 0xfc, 0x44, 0xbf, 0xe3, 0x41, 0x6f, 0x7c, 0x3b, 0xb6, 0x3d,
            0x08, 0x67, 0xe8, 0x38, 0xf2, 0xa4, 0xb8, 0xf1, 0xd6, 0xc1, 0x33, 0x1c, 0x79, 0x1d,
            0xf4, 0x41, 0x63, 0x81, 0x38, 0xfe, 0xe0, 0x1d, 0x7e, 0x14, 0x3f, 0x44, 0xfd, 0xb8,
            0x74, 0x5c, 0x48, 0x2f, 0x7c, 0x87, 0x37, 0xc7, 0x45, 0x8f, 0xfc, 0x87, 0x96, 0xde,
            0x41, 0x9b, 0x06, 0x07, 0x99, 0x69, 0x68, 0xa6, 0xe3, 0x47, 0xfb, 0x42, 0xe8, 0x83,
            0xc7, 0x88, 0x5e, 0xe4, 0x37, 0x6e, 0xb8, 0xce, 0xa1, 0x1d, 0x17, 0x4e, 0xfc, 0x91,
            0x84, 0xa6, 0x4d, 0x45, 0xfc, 0xf8, 0x71, 0x4a, 0x86, 0x8e, 0x1f, 0x5e, 0x09, 0xc8,
            0x62, 0xf0, 0xc3, 0x38, 0x7e, 0x08, 0x2f, 0x8f, 0x7c, 0xb8, 0x8e, 0xcb, 0xf8, 0x73,
            0xfc, 0x38, 0x83, 0x17, 0xb6, 0x90, 0x30, 0x3c, 0x72, 0xe6, 0xe8, 0x85, 0xeb, 0xf8,
            0x82, 0x17, 0x8f, 0x22, 0x24, 0x8f, 0x4d, 0x44, 0x66, 0xf2, 0x61, 0x32, 0x9e, 0xc3,
            0xba, 0xf0, 0xe3, 0x45, 0xbf, 0xe3, 0x39, 0xfa, 0xc2, 0x3f, 0x42, 0x66, 0x3c, 0x94,
            0x33, 0x21, 0x7e, 0xe4, 0xb8, 0x25, 0xe3, 0x49, 0x8e, 0x6b, 0x0f, 0x9e, 0x08, 0x78,
            0x90, 0xed, 0x45, 0x73, 0x54, 0x01, 0x7e, 0xf4, 0x57, 0x21, 0x7a, 0x0f, 0x7c, 0x1c,
            0x67, 0xa0, 0xf6, 0x70, 0x51, 0x34, 0xe7, 0x89, 0x2e, 0xb2, 0xf0, 0xe3, 0x3b, 0x9a,
            0x3c, 0x81, 0xf6, 0x09, 0xcf, 0x8f, 0xdc, 0xca, 0x83, 0x8a, 0xc5, 0x27, 0x11, 0x9f,
            0x83, 0x4a, 0xc7, 0x6f, 0xe1, 0x7f, 0xd0, 0x74, 0x48, 0x76, 0xed, 0xc8, 0x8d, 0xb2,
            0x4b, 0x83, 0x86, 0x17, 0xfe, 0x83, 0x3f, 0x2e, 0xe5, 0x48, 0x56, 0x1d, 0x79, 0x0f,
            0x2b, 0xc7, 0xa3, 0x1c, 0xd3, 0x51, 0x1e, 0x37, 0x5e, 0xfc, 0x47, 0xd8, 0xe3, 0x0f,
            0xb4, 0x5f, 0x14, 0x1a, 0x25, 0x23, 0x22, 0x93, 0xc1, 0x7d, 0x1c, 0x97, 0x24, 0xe2,
            0xd1, 0x51, 0xfe, 0x41, 0x99, 0xec, 0x78, 0xf0, 0x07, 0xf9, 0x8b, 0xca, 0x68, 0xc6,
            0xe0, 0xd0, 0x8f, 0x2f, 0x45, 0xaf, 0xa0, 0xc2, 0xa5, 0x29, 0xc6, 0x71, 0xe8, 0xc7,
            0x8f, 0x5f, 0x50, 0x7e, 0x5c, 0x3a, 0xc2, 0x1f, 0x1e, 0xda, 0x45, 0xf1, 0x70, 0x0a,
            0xfd, 0x71, 0x31, 0xc8, 0x7f, 0x68, 0x1c, 0xe2, 0x07, 0xa1, 0x5f, 0xfc, 0x82, 0x8f,
            0x8f, 0x50, 0xfe, 0x21, 0x6d, 0x85, 0x96, 0x47, 0xfe, 0x0e, 0x17, 0xeb, 0x43, 0x67,
            0x96, 0x1a, 0x2f, 0x8f, 0x0b, 0xf5, 0x05, 0xbe, 0x81, 0x5c, 0x09, 0xbd, 0x91, 0x47,
            0x68, 0xf6, 0xe3, 0x15, 0xf2, 0x1e, 0xae, 0xa4, 0x3c, 0xd0, 0x78, 0xe1, 0x3c, 0xca,
            0x1e, 0x69, 0x17, 0x05, 0x3d, 0x42, 0x8e, 0x87, 0x9e, 0x1d, 0xd6, 0x11, 0xfe, 0xb8,
            0x85, 0x5e, 0x87, 0xfb, 0xe0, 0x2a, 0x2e, 0x21, 0x3d, 0xc7, 0x11, 0x5a, 0x7a, 0x9c,
            0xc3, 0x87, 0xe3, 0x60, 0x9e, 0x0a, 0x22, 0x7e, 0xf8, 0x3d, 0x72, 0xdc, 0x11, 0xde,
            0xe5, 0xc8, 0xae, 0x1d, 0x5f, 0x0e, 0x49, 0xd3, 0x1e, 0x94, 0x11, 0x2e, 0x82, 0xc9,
            0x57, 0xf4, 0x49, 0xf0, 0x37, 0x28, 0x97, 0x1c, 0x14, 0xc7, 0x13, 0x67, 0x8e, 0xe3,
            0x39, 0x3e, 0x1e, 0xf8, 0x70, 0x4c, 0x3f, 0x8e, 0x67, 0x32, 0x4a, 0x3d, 0x22, 0xb6,
            0x23, 0xef, 0x87, 0x06, 0xc7, 0x3d, 0x34, 0x47, 0x9f, 0x23, 0x0c, 0x3f, 0xf4, 0x1f,
            0xf4, 0x23, 0xcc, 0x89, 0x2a, 0x47, 0xe5, 0x5c, 0xf0, 0x8b, 0x67, 0x85, 0x7f, 0xa0,
            0xc7, 0x8d, 0x30, 0x51, 0x96, 0x24, 0x4e, 0x06, 0x00, 0x00, 0x00, 0x00, 0x44, 0x00,
            0x21, 0x4a, 0x53, 0x03, 0x00, 0x20, 0x42, 0x10, 0xc1, 0x10, 0x10, 0x86, 0x28, 0x43,
            0x01, 0x03, 0x46, 0x01, 0x65, 0x94, 0x73, 0x10, 0x10, 0x46, 0x24, 0x55, 0x02, 0x11,
            0x01, 0x01, 0x62, 0x58, 0x50, 0x03, 0x84, 0x00, 0xc2, 0x01, 0xc5, 0x8c, 0x14, 0x0e,
            0x22, 0x61, 0x9c, 0x22, 0x46, 0x00, 0x21, 0x10, 0x11, 0xc2, 0x00, 0x26, 0x18, 0x53,
            0x4e, 0x11, 0x01, 0x2e, 0x72, 0x82, 0x53, 0x48, 0x0a, 0x01, 0x49, 0xb1, 0x28, 0x04,
            0x11, 0x16, 0x18, 0x61, 0x90, 0x44, 0x0e, 0x28, 0x21, 0x08, 0x70, 0x80, 0x02, 0xa7,
            0x0c, 0x31, 0x80, 0x32, 0xa5, 0x88, 0x50, 0x44, 0x31, 0x67, 0x00, 0x62, 0x84, 0x40,
            0x04, 0x88, 0x03, 0xce, 0x18, 0x48, 0x00, 0xb1, 0x0e, 0x4c, 0x0b, 0x24, 0x60, 0xd4,
            0x18, 0x23, 0xa8, 0x92, 0xcc, 0x10, 0x22, 0x8c, 0xc2, 0x82, 0x38, 0xe3, 0xa8, 0x02,
            0x10, 0x10, 0x44, 0x40, 0x12, 0x44, 0x0b, 0x43, 0x0c, 0x35, 0x4a, 0x08, 0xa5, 0x10,
            0x31, 0x84, 0x00, 0x21, 0x80, 0x12, 0xd0, 0x00, 0x42, 0x00, 0x60, 0x86, 0x01, 0x43,
            0x00, 0x60, 0x00, 0x18, 0x00, 0x1d, 0x50, 0x94, 0x32, 0x04, 0x14, 0x20, 0x8e, 0x10,
            0x41, 0x18, 0x82, 0x40, 0x08, 0xaa, 0x90, 0x02, 0x86, 0x15, 0xac, 0x85, 0xa0, 0x0e,
            0x01, 0x22, 0x19, 0x22, 0x0c, 0x09, 0x25, 0x88, 0x02, 0x08, 0x00, 0x41, 0xa0, 0x61,
            0x16, 0x11, 0x45, 0x94, 0x01, 0x00, 0x20, 0x60, 0x14, 0x64, 0x4c, 0x00, 0x04, 0x0c,
            0x70, 0xc6, 0x40, 0x25, 0x04, 0xb2, 0xca, 0x58, 0xa2, 0x98, 0x42, 0x40, 0x09, 0x86,
            0xa1, 0x22, 0x14, 0x00, 0x23, 0x04, 0x20, 0xc0, 0x09, 0x43, 0x09, 0x51, 0x40, 0x09,
            0x62, 0x80, 0x32, 0x48, 0x39, 0x40, 0x20, 0x43, 0x40, 0x03, 0xc2, 0x01, 0x11, 0x08,
            0x09, 0x02, 0x04, 0x32, 0x00, 0x30, 0xe0, 0xa4, 0x03,
        ];

        let test_compressed = compress_fingerprint(raw.as_slice(), 1);

        assert_eq!(
            test_compressed, known_compressed,
            "Expected compressed fingerprint to match a known reference"
        );
    }
}
