use crate::compression::pack::pack3_size;

use super::pack::{unpack3, unpack5};
use std::{fmt::Display, collections::VecDeque};

#[derive(Debug)]
pub enum DecompressError {
    InputTooShort(usize),
    UnexpectedEndOfData,
    MissingExtension(usize, usize),
    IncompleteStride,
}

impl Display for DecompressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooShort(len) => write!(
                f,
                "Input is too short to be a valid fingerprint. Found {len} bytes, but expected >4."
            ),
            Self::UnexpectedEndOfData => write!(
                f,
                "The fingerprint does not contain enough data to be completely decompressed"
            ),
            Self::MissingExtension(expected, actual) => write!(
                f,
                "Expected {expected} extensions, but {actual} were present in the input."
            ),
            Self::IncompleteStride => write!(f, "Final stride did not terminate."),
        }
    }
}

impl std::error::Error for DecompressError {}

pub fn decompress_fingerprint(
    compressed: &[u8],
) -> Result<(u8, Vec<u32>), DecompressError> {
    if compressed.len() < 4 {
        return Err(DecompressError::InputTooShort(compressed.len()));
    }

    let mut cursor = compressed.iter();

    let algorithm = cursor.next().unwrap();
    let length = (((*cursor.next().unwrap() as usize) << 16)
        | ((*cursor.next().unwrap() as usize) << 8)
        | (*cursor.next().unwrap() as usize)) as usize;

    if length > compressed.len() - 4 {
        return Err(DecompressError::UnexpectedEndOfData);
    }

    // we don't know how many extensions are present up-front, so
    // we'll treat the entirety of the remaining data as if it were
    // normal spans.
    let mut spans = unpack3(&compressed[4..]);

    // walk possible spans until we have discovered the true quantity,
    // slightly abuse try_fold to stop counting once we reach the final span
    let last_span = spans
        .iter()
        .enumerate()
        .try_fold((0usize, 0usize), |(mut elem_count, ext_count), (index, span)| {
            match *span {
                0 => {
                    elem_count += 1;

                    if elem_count == length {
                        Err((index, ext_count))
                    } else {
                        Ok((elem_count, ext_count))
                    }
                },
                7 => {
                    Ok((elem_count, ext_count + 1))
                },
                _ => Ok((elem_count, ext_count))
            }
        });

    let (last_span, expect_exts) = match last_span {
        Err(stats) => stats,
        // if we ran out of spans before we counted enough strides to
        // reassemble the input, it means that the compressed data is
        // incomplete or invalid.
        Ok(_) => return Err(DecompressError::UnexpectedEndOfData),
    };

    spans.resize(last_span + 1, 0);

    let ext_offset = 4 + pack3_size(spans.len());

    let mut exts = unpack5(&compressed[ext_offset..]);

    if exts.len() < expect_exts {
        return Err(DecompressError::MissingExtension(expect_exts, exts.len()));
    }

    let mut out: Vec<u32> = Vec::with_capacity(length);

    // the distance from the LSB in the fingerprint under construction,
    let mut bit_offset = 0u8;
    let mut fp_prev = 0u32;
    let mut fp = 0u32;

    while let Some(span) = spans.pop_front() {
        match span {
            0 => {
                fp ^= fp_prev;
                out.push(fp);
                fp_prev = fp;
                fp = 0;
                bit_offset = 0;
            },
            7 => {
                // we know that this unwrap should not panic, as we
                // counted the number of extended spans (7s) in the
                // input earlier, and returned early if too few
                // extensions were found in the input
                bit_offset += 7 + exts.pop_front().unwrap();
                fp |= 1 << (bit_offset - 1);
            },
            span => {
                bit_offset += span;
                fp |= 1 << (bit_offset - 1);
            }
        }
    }

    if fp != 0 {
        Err(DecompressError::IncompleteStride)
    } else {
        Ok((*algorithm, out))
    }
}

#[cfg(test)]
mod tests {
    use super::super::compress_fingerprint;
    use super::*;

    // official test suite

    #[test]
    fn one_item_one_bit() {
        let expected = vec![1u32];
        let compressed = [0u8, 0, 0, 1, 1];

        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Algorithm must match input");
        assert_eq!(decompressed, expected, "Decompressed output did not match expected output");
    }

    #[test]
    fn one_item_three_bits() {
        let expected = vec![7u32];
        let compressed = [0u8, 0, 0, 1, 73, 0];

        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Algorithm must match input");
        assert_eq!(decompressed, expected, "Decompressed output did not match expected output");
    }

    #[test]
    fn one_item_one_bit_except() {
        let expected = vec![1u32 << 6];
        let compressed = [0u8, 0, 0, 1, 7, 0];

        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Algorithm must match input");
        assert_eq!(decompressed, expected, "Decompressed output did not match expected output");
    }

    #[test]
    fn one_item_one_bit_except_2() {
        let expected = vec![1u32 << 8];
        let compressed = [0u8, 0, 0, 1, 7, 2];

        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Algorithm must match input");
        assert_eq!(decompressed, expected, "Decompressed output did not match expected output");
    }

    #[test]
    fn two_items() {
        let expected = vec![1, 0];
        let compressed = [0u8, 0, 0, 2, 65, 0];

        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Algorithm must match input");
        assert_eq!(decompressed, expected, "Decompressed output did not match expected output");
    }

    #[test]
    fn two_items_no_change() {
        let expected = vec![1, 1];
        let compressed = [0u8, 0, 0, 2, 1, 0];

        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Algorithm must match input");
        assert_eq!(decompressed, expected, "Decompressed output did not match expected output");
    }

    #[test]
    fn invalid_1() {
        let compressed = [0u8, 255, 255, 255];

        decompress_fingerprint(&compressed).expect_err("Fingerprint is too short to decompress");
    }

    // other tests

    #[test]
    fn round_trip() {
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

        let compressed = compress_fingerprint(&raw, 0);
        let (algo, decompressed) = decompress_fingerprint(&compressed).unwrap();

        assert_eq!(algo, 0, "Extracted algorithm must match original");
        assert_eq!(decompressed, raw, "Decompressed output must much input to compressor");
    }
}
