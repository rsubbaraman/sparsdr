/*
 * Copyright 2021 The Regents of the University of California
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

//!
//! Reconstructs with multiple bands and checks that an initial gap is added to each output file
//!

extern crate byteorder;
extern crate num_complex;
extern crate num_traits;
extern crate sparsdr_reconstruct;
extern crate sparsdr_sample_parser;

use byteorder::{ReadBytesExt, LE};
use num_complex::Complex;
use num_traits::Zero;

use sparsdr_reconstruct::steps::overlap::OverlapMode;
use sparsdr_reconstruct::window::Window;
use sparsdr_reconstruct::{decompress, BandSetupBuilder, DecompressSetup};

#[test]
fn test_initial_gap() -> Result<(), Box<dyn std::error::Error>> {
    let fft_size = 2048;
    let timestamp_bits = 20;
    let compressed_bandwidth = 100e6;

    let windows_in: Vec<Window> = vec![
        // Window, time 10, bins 0 and 1 active
        Window::with_bins(10, fft_size, {
            let mut bins = vec![Complex::zero(); fft_size];
            bins[0] = Complex::new(0.5, 0.5);
            bins[1] = Complex::new(0.5, 0.5);
            bins
        }),
        // Two samples, time 20, bins 1999 and 2000
        Window::with_bins(20, fft_size, {
            let mut bins = vec![Complex::zero(); fft_size];
            bins[1999] = Complex::new(-0.5, -0.5);
            bins[2000] = Complex::new(-0.5, -0.5);
            bins
        }),
        // Two samples, time 30, bins 3 and 4
        Window::with_bins(30, fft_size, {
            let mut bins = vec![Complex::zero(); fft_size];
            bins[3] = Complex::new(0.5, 0.5);
            bins[4] = Complex::new(0.5, 0.5);
            bins
        }),
    ];

    // Output bytes for the lower frequency range (corresponding to bins 1024..2048)
    let mut samples_lower_half: Vec<u8> = Vec::new();
    // Output bytes for the higher frequency range (corresponding to bins 0..1024)
    let mut samples_upper_half: Vec<u8> = Vec::new();

    let mut setup = DecompressSetup::new(
        windows_in.into_iter().map(std::io::Result::Ok),
        fft_size,
        timestamp_bits,
    );
    setup.set_overlap_mode(OverlapMode::Gaps);
    setup.add_band(
        BandSetupBuilder::new(
            Box::new(&mut samples_lower_half),
            compressed_bandwidth,
            fft_size,
            1024,
            1024,
        )
        .center_frequency(-25e6)
        .build(),
    );
    setup.add_band(
        BandSetupBuilder::new(
            Box::new(&mut samples_upper_half),
            compressed_bandwidth,
            fft_size,
            1024,
            1024,
        )
        .center_frequency(25e6)
        .build(),
    );

    decompress(setup)?;

    println!("Lower half {} bytes", samples_lower_half.len());
    println!("Upper half {} bytes", samples_upper_half.len());

    // Lower half gets 10 half-windows of zeros, and then a full window of non-zero samples
    assert_eq!(
        samples_lower_half.len(),
        8 * 512 * 10 + 8 * 1024,
        "Lower half incorrect number of samples"
    );
    {
        let mut lower_half_bytes = samples_lower_half.as_slice();
        for _ in 0..512 * 10 {
            let real = lower_half_bytes.read_f32::<LE>()?;
            let imaginary = lower_half_bytes.read_f32::<LE>()?;
            assert_eq!(real, 0.0, "Non-zero sample in gap");
            assert_eq!(imaginary, 0.0, "Non-zero sample in gap");
        }
    }

    // Upper half gets a full window of non-zero samples, 18 half-windows
    // of zeros, and a full window of non-zero samples
    assert_eq!(
        samples_upper_half.len(),
        8 * 1024 + 8 * 512 * 18 + 8 * 1024,
        "Upper half incorrect number of samples"
    );
    for byte in &samples_upper_half[8 * 1024..][..8 * 512 * 18] {
        assert_eq!(*byte, 0, "Non-zero byte in gap");
    }
    {
        let mut upper_half_bytes = &samples_upper_half.as_slice()[8 * 1024..];
        for _ in 0..512 * 18 {
            let real = upper_half_bytes.read_f32::<LE>()?;
            let imaginary = upper_half_bytes.read_f32::<LE>()?;
            assert_eq!(real, 0.0, "Non-zero sample in gap");
            assert_eq!(imaginary, 0.0, "Non-zero sample in gap");
        }
    }
    Ok(())
}
