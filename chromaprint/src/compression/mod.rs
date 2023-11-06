// How does Chromaprint compression work?
//
// Each Chromaprint fingerprint is a vector of 32-bit subfingerprints.
// Chromprint compression works exclusively over these
// subfingerprints.
//
// The subfingerprint compression strategy is fairly simple: first, a
// subfingerprint is XORed with the preceeding subfingerprint (or 0,
// in the case of the first), and then the XORed subfingerprint is
// encoded as a sequence of lengths between set (1) bits. The
// rationale for the XOR stage is not given; however, it is most
// likely done to reduce the number of set bits in a given
// subfingerprint, thereby reducing the number of spans needed to
// represent that subfingerprint. The spans themselves are stored as
// three bits, which is sufficient for most cases - where a span is
// longer than 7 bits, the remaining length is stored in an extension
// span, which holds five bits.
//
// Example: the following 32-bit number is reduced to the given series
// of spans:
//
//   Subfingerprint: 0000 0000 0110 0000 1000 0011 0010 0000 (6325024)
//   Spans: {6, 3, 1, 6, 6, 1}
//
// This may be visualized as below, where each ↑ marks the end of a
// span,
//
//   32                              1
//    00000000011000001000001100100000
//             16543216543211321654321
//             ↑↑     ↑     ↑↑   ↑
//
// In the case of the above number, we've achieved a decent
// compression ratio, representing a 32-bit number in 9 bits.
//
// When a span exceeds 7 bits in length, the remainder of the length
// is placed into an extension cell, e.g.
//
//   Subfingerprint: 0000 0000 0000 0000 0000 0000 1000 0000
//   Spans: {7}
//   Extensions: {1}
//
// And when a span is exactly 7 bits in length, a zero is inserted
// into the extension sequence,
//
//   Subfingerprint: 0000 0000 0000 0000 0000 0000 0100 0000
//   Spans: {7}
//   Extensions: {0}
//
// Each span sequence is terminated with a 0. This ensures that the
// decompressor knows when to flush the subfingerprint. For instance,
// the following span and extension sequence encodes two
// subfingerprints,
//
//   Spans: {6, 3, 1, 6, 7}
//   Extensions: {1}
//
// Decompression is a straightforward operation: the span sequence and
// corresponding extension sequence represent a reverse-ordered
// sequence of shift operations. For instance,
//
//   Spans: {6, 3, 1, 6, 6, 1}
//
//   [0]: 1 << 1         →                            10
//   [1]: ([0] | 1) << 6 →                     1100 0000
//   [2]: ([1] | 1) << 6 →             11 0000 0100 0000
//   [3]: ([2] | 1) << 1 →           0110 0000 1000 0010
//   [4]: ([3] | 1) << 3 →        11 0000 0100 0001 1000
//   [5]: ([4] | 1) << 6 → 1100 0001 0000 0110 0100 0000
//   [6]: [5] >> 1       →  110 0000 1000 0011 0010 0000
//
// In the above sequence of operations, operation [6] gives the
// original input to the span encoder.
//
// The official decompression method is a little bit different, as it
// works with the spans in forward order:
//
//   Spans: {6, 3, 1, 6, 6, 1}
//
//   [0] {bc=6, bp= 0}:        1 << ((bp += bc) - 1 = 5)  →                      10 0000
//   [1] {bc=3, bp= 6}: [0] | (1 << ((bp += bc) - 1 = 8)  →                  1 0010 0000
//   [2] {bc=1, bp= 9}: [1] | (1 << ((bp += bc) - 1 = 9)  →                 11 0010 0000
//   [3] {bc=6, bp=10}: [2] | (1 << ((bp += bc) - 1 = 15) →          1000 0011 0010 0000
//   [4] {bc=6, bp=16}: [3] | (1 << ((bp += bc) - 1 = 21) →  10 0000 1000 0011 0010 0000
//   [5] {bc=1, bp=22}: [4] | (1 << ((bp += bc) - 1 = 22) → 110 0000 1000 0011 0010 0000

mod pack;
mod decompress;
mod compress;

pub use decompress::decompress_fingerprint;
pub use compress::compress_fingerprint;
