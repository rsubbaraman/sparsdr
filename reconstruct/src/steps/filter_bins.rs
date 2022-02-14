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

//! Bin filtering

use num_complex::Complex32;
use num_traits::Zero;
use std::convert::TryFrom;

use crate::bins::BinRange;
use crate::window::{Logical, Status, Window};

/// Filters Samples and applies an offset to index values
struct FilterBins {
    /// Bin range to map from
    bins: BinRange,
    /// Index values get mapped into a region of bins of the same size as bins, at the center of
    /// 0..fft_size
    fft_size: u16,
}

impl FilterBins {
    fn new(bins: BinRange, fft_size: u16) -> Self {
        debug_assert!(bins.end() >= bins.start());
        assert!(bins.size() <= fft_size);
        FilterBins { bins, fft_size }
    }

    /// Filters a window and moves the bins to new positions in the window
    ///
    /// Returns None if the window contains no active bins in this filter's bin range
    fn filter_window(&mut self, mut window: Window<Logical>) -> Option<Window<Logical>> {
        debug_assert!(usize::from(self.bins.end()) <= window.bins().len());
        let matches = matches_range(&self.bins, window.bins());
        if matches {
            // Need to truncate the window to self.fft_size bins, and move values in the range
            // self.bins to the center of the truncated window

            // Part 1: Zero out everything that is outside the range
            // Before start
            let before_start = ..usize::from(self.bins.start());
            for bin in &mut window.bins_mut()[before_start] {
                *bin = Complex32::zero();
            }
            // After end
            let after_end = usize::from(self.bins.end())..;
            for bin in &mut window.bins_mut()[after_end] {
                *bin = Complex32::zero();
            }

            // Shift
            let offset = i16::try_from(self.bins.middle()).unwrap()
                - i16::try_from(self.fft_size / 2).unwrap();
            log::debug!(
                "filter_window, self.bins = {}, self.fft_size = {}, window size = {}, offset = {}",
                self.bins,
                self.fft_size,
                window.bins().len(),
                offset
            );
            if offset >= 0 {
                window.bins_mut().rotate_left(offset as usize);
            } else {
                window.bins_mut().rotate_right((-offset) as usize);
            }

            // Truncate
            window.truncate_bins(usize::from(self.fft_size));

            Some(window)
        } else {
            None
        }
    }
}

/// Returns true if any value at an index within the provided bin range is non-zero
fn matches_range(range: &BinRange, bins: &[Complex32]) -> bool {
    bins[range.as_usize_range()].iter().any(|v| !v.is_zero())
}

/// An iterator adapter that filters bins of windows
pub struct FilterBinsIter<I> {
    /// Inner iterator
    inner: I,
    /// Filter
    filter: FilterBins,
}

impl<I> FilterBinsIter<I> {
    /// Creates a window-bin-filtering iterator adapter
    pub fn new(inner: I, bins: BinRange, fft_size: u16) -> Self {
        FilterBinsIter {
            inner,
            filter: FilterBins::new(bins, fft_size),
        }
    }
}

impl<I> Iterator for FilterBinsIter<I>
where
    I: Iterator<Item = Status<Window<Logical>>>,
{
    type Item = Status<Window<Logical>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let window: Window<Logical> = try_status!(self.inner.next());

            if let Some(filtered) = self.filter.filter_window(window) {
                return Some(Status::Ok(filtered));
            } else {
                // Continue and look for the next window
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_windows_out_of_range() {
        let mut window = Window::new_logical(0, 2048);
        // Put some things in the lower half
        {
            let bins = window.bins_mut();
            bins[0] = Complex32::new(3.0, 4.0);
            bins[1023] = Complex32::new(5.0, 6.0);
        }
        let bins = BinRange::from(1024..2047);
        let fft_size = 1024;
        check_windows(bins, fft_size, window, None);
    }

    #[test]
    fn test_windows_one_bin() {
        let mut window = Window::new_logical(0, 2048);
        // Put some things in the lower half
        {
            let bins = window.bins_mut();
            bins[0] = Complex32::new(3.0, 4.0);
            bins[1023] = Complex32::new(5.0, 6.0);
        }

        let expected = {
            let mut expected = Window::new_logical(0, 1);
            expected.bins_mut()[0] = Complex32::new(5.0, 6.0);
            expected
        };

        let bins = BinRange::from(1023..1024);
        let fft_size = 1;
        check_windows(bins, fft_size, window, Some(expected));
    }

    #[test]
    fn test_windows_centered_short() {
        // Keep the 4 bins in the middle of the 8
        let bins = BinRange::from(2..6);
        let fft_size = 4;
        let window = {
            let mut window = Window::new_logical(0, 8);
            {
                let bins = window.bins_mut();
                // Put some things in the middle
                bins[3] = Complex32::new(3.0, 4.0);
                bins[4] = Complex32::new(5.0, 6.0);
                // Put some things just inside the middle 4
                bins[5] = Complex32::new(7.0, 8.0);
                // Put some things just outside the middle 4
                bins[6] = Complex32::new(33.0, 621.0);
                bins[7] = Complex32::new(44.0, 45.0);
                // Put something at the start
                bins[0] = Complex32::new(1.0, 2.0);
            }
            window
        };
        let expected = {
            let mut expected = Window::new_logical(0, 4);
            {
                let bins = expected.bins_mut();
                // Those two nonzero values should stay at the center
                bins[1] = Complex32::new(3.0, 4.0);
                bins[2] = Complex32::new(5.0, 6.0);
                // The one other nonzero thing should be at the end
                bins[3] = Complex32::new(7.0, 8.0);
            }
            expected
        };
        check_windows(bins, fft_size, window, Some(expected));
    }

    #[test]
    fn test_windows_centered_short_larger_fft() {
        // Keep the 4 bins in the middle of the 8
        let bins = BinRange::from(2..6);
        // Since the FFT size is 8, the resulting window should have size 8 but should only include
        // the 4 selected bins
        let fft_size = 8;
        let window = {
            let mut window = Window::new_logical(0, 8);
            {
                let bins = window.bins_mut();
                // Put some things in the middle
                bins[3] = Complex32::new(3.0, 4.0);
                bins[4] = Complex32::new(5.0, 6.0);
                // Put some things just inside the middle 4
                bins[5] = Complex32::new(7.0, 8.0);
                // Put some things just outside the middle 4
                bins[6] = Complex32::new(33.0, 621.0);
                bins[7] = Complex32::new(44.0, 45.0);
                // Put something at the start
                bins[0] = Complex32::new(1.0, 2.0);
            }
            window
        };
        let expected = {
            let mut expected = Window::new_logical(0, 8);
            {
                let bins = expected.bins_mut();
                // Those two nonzero values should stay at the center
                bins[3] = Complex32::new(3.0, 4.0);
                bins[4] = Complex32::new(5.0, 6.0);
                // The one other nonzero thing should be at the end
                bins[5] = Complex32::new(7.0, 8.0);
            }
            expected
        };
        check_windows(bins, fft_size, window, Some(expected));
    }

    #[test]
    fn test_windows_centered() {
        // Keep the 1024 bins in the middle of the 2048
        let bins = BinRange::from(512..1536);
        let fft_size = 1024;
        let window = {
            let mut window = Window::new_logical(0, 2048);
            {
                let bins = window.bins_mut();
                // Put some things in the middle
                bins[1023] = Complex32::new(3.0, 4.0);
                bins[1024] = Complex32::new(5.0, 6.0);
                // Put some things at the ends
                bins[0] = Complex32::new(1.0, 2.0);
                bins[2047] = Complex32::new(9.0, 10.0);
            }
            window
        };
        let expected = {
            let mut expected = Window::new_logical(0, 1024);
            {
                let bins = expected.bins_mut();
                // Those two nonzero values should stay at the center
                bins[511] = Complex32::new(3.0, 4.0);
                bins[512] = Complex32::new(5.0, 6.0);
            }
            expected
        };
        check_windows(bins, fft_size, window, Some(expected));
    }

    #[test]
    fn test_large_fft_before_start() {
        // 50 bins centered at 15 would run past the beginning of the FFT, but this should
        // still work.
        let bins = BinRange::from(10..20);
        let fft_size = 50u16;
        let window = {
            let mut window = Window::new_logical(0, 512);
            {
                let bins = window.bins_mut();
                bins[9] = Complex32::new(0.0, 9.0);
                // These will show up in the output
                bins[10] = Complex32::new(0.0, 10.0);
                bins[11] = Complex32::new(0.0, 11.0);
                bins[12] = Complex32::new(0.0, 12.0);
                bins[13] = Complex32::new(0.0, 13.0);
                bins[14] = Complex32::new(0.0, 14.0);
                bins[15] = Complex32::new(0.0, 15.0);
                bins[16] = Complex32::new(0.0, 16.0);
                bins[17] = Complex32::new(0.0, 17.0);
                bins[18] = Complex32::new(0.0, 18.0);
                bins[19] = Complex32::new(0.0, 19.0);
                // These will not show up in the output
                bins[20] = Complex32::new(0.0, 20.0);
            }
            window
        };
        let expected = {
            let mut expected = Window::new_logical(0, fft_size.into());
            {
                let bins = expected.bins_mut();
                // Bins 10..20 become 20..30
                bins[20] = Complex32::new(0.0, 10.0);
                bins[21] = Complex32::new(0.0, 11.0);
                bins[22] = Complex32::new(0.0, 12.0);
                bins[23] = Complex32::new(0.0, 13.0);
                bins[24] = Complex32::new(0.0, 14.0);
                bins[25] = Complex32::new(0.0, 15.0);
                bins[26] = Complex32::new(0.0, 16.0);
                bins[27] = Complex32::new(0.0, 17.0);
                bins[28] = Complex32::new(0.0, 18.0);
                bins[29] = Complex32::new(0.0, 19.0);
            }
            expected
        };
        check_windows(bins, fft_size, window, Some(expected));
    }

    fn check_windows(
        bins: BinRange,
        fft_size: u16,
        input: Window<Logical>,
        expected: Option<Window<Logical>>,
    ) {
        let mut filter = FilterBins::new(bins, fft_size);

        let actual_window = filter.filter_window(input);
        if actual_window != expected {
            if let (Some(actual), Some(expected)) = (actual_window, expected) {
                println!("Expected: {}", expected.show_non_empty());
                println!("Actual:   {}", actual.show_non_empty());
            }
            panic!("Windows don't match");
        }
    }
}
