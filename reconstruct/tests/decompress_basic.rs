/*
 * Copyright 2019 The Regents of the University of California
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 */

extern crate byteorder;
extern crate num_complex;
extern crate simplelog;
extern crate sparsdr_reconstruct;

use std::iter;

use sparsdr_reconstruct::{decompress, BandSetupBuilder, DecompressSetup};

mod test_vectors;

#[test]
fn test_empty() {
    let empty_source = iter::empty();
    let mut destination = Vec::new();
    {
        let band_setup = BandSetupBuilder::new(Box::new(&mut destination), 100e6, 2048, 2048, 2048)
            .bins(2048)
            .center_frequency(0.0);
        let mut setup = DecompressSetup::new(empty_source, 2048, 20);
        setup.add_band(band_setup.build());
        decompress(setup).expect("Decompress failed");
    }
    assert!(destination.is_empty());
}

/// Simple decompression with all 2048 bins
#[test]
fn test_small_2048_bins() {
    test_vectors::test_with_vectors(
        "test-data/all-2048/STFT1_testvectors_0fc",
        "test-data/all-2048/x_istft_f_testvectors_0fc-32",
        "test-data/all-2048/decompressed.iq",
        0.0,
        2048,
    );
}

/// Simple decompression with fewer than 2048 bins, but with no frequency offset
#[test]
fn test_fewer_bins_on_center() {
    test_vectors::test_with_vectors(
        "test-data/fewer-centered/STFT_testvectors_0fc",
        "test-data/fewer-centered/x_istft_f_testvectors_0fc-32",
        "test-data/fewer-centered/decompressed.iq",
        0.0,
        64,
    );
}

/// Decompression with fewer than 2048 bins and an on-bin frequency offset
///
/// Center frequency to be decompressed is 64 bins beyond the original center frequency
// FIXME bin and non-bin have their names reversed
#[test]
fn test_bin_frequency_offset() {
    test_vectors::test_with_vectors(
        "test-data/bin-frequency-offset/STFT_testvectors_0fc",
        "test-data/bin-frequency-offset/x_istft_f_testvectors_0fc-32",
        "test-data/bin-frequency-offset/decompressed.iq",
        64.5 * 100e6 / 2048.0,
        64,
    );
}

/// Decompression with fewer than 2048 bins and some other frequency offset
///
/// Center frequency to be decompressed is 64 bins beyond the original center frequency
#[test]
fn test_non_bin_frequency_offset() {
    test_vectors::test_with_vectors(
        "test-data/non-bin-frequency-offset/STFT_testvectors_0fc",
        "test-data/non-bin-frequency-offset/x_istft_f_testvectors_0fc-32",
        "test-data/non-bin-frequency-offset/decompressed.iq",
        64.0 * 100e6 / 2048.0,
        64,
    );
}

#[test]
fn test_large_offset() {
    test_vectors::test_with_vectors(
        "test-data/500p5/STFT_testvectors_0fc",
        "test-data/500p5/x_istft_f_testvectors_0fc-32",
        "test-data/500p5/decompressed.iq",
        500.5 * 100e6 / 2048.0,
        64,
    );
}

#[test]
fn test_random_offset() {
    test_vectors::test_with_vectors(
        "test-data/random-offset/STFT_testvectors_0fc",
        "test-data/random-offset/x_istft_f_testvectors_0fc-32",
        "test-data/random-offset/decompressed.iq",
        100e6 / 5.0,
        64,
    );
}
