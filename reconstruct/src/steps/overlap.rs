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

//! The overlap step

use std::iter::Fuse;

use num_complex::Complex32;
use num_traits::Zero;

use crate::window::{Status, TimeWindow};

/// Modifies `first_second_half` by adding to each element the corresponding value from the first
/// half of `second`
fn overlap_windows(first_second_half: &mut TimeWindow, second: &TimeWindow) {
    assert_eq!(first_second_half.len(), second.len() / 2);
    for (first_value, second_value) in first_second_half
        .samples_mut()
        .iter_mut()
        .zip(second.first_half().iter())
    {
        *first_value += *second_value;
    }
}

/// Overlap states
pub enum State {
    /// Nothing happening
    Idle,
    /// Producing a gap between windows
    Gap {
        /// Number of half-windows to produce before the next window (may be zero)
        remaining_half_windows: u64,
        /// The next data window (full-length)
        next_window: TimeWindow,
    },
    /// Producing overlapped windows
    Overlap {
        /// The second half of the previous window (the first half has already been produced)
        previous_half: TimeWindow,
    },
}

/// An iterator adapter that overlaps windows
///
/// This implementation includes gaps between samples. Due to timestamp overflow, gaps longer than
/// about 10 seconds cannot be represented.
pub struct Overlap<I> {
    /// Inner iterator
    inner: Fuse<I>,
    /// Window size, samples
    window_size: usize,
    /// Current state
    state: State,
}

impl<I> Overlap<I>
where
    I: Iterator<Item = Status<TimeWindow>>,
{
    /// Creates an overlap iterator for the provided window size
    pub fn new(inner: I, window_size: usize) -> Self {
        Overlap {
            inner: inner.fuse(),
            window_size,
            state: State::Idle,
        }
    }

    /// Repeatedly calls self.inner.next(), ignoring any timeouts
    fn wait_for_next_window(&mut self) -> Option<TimeWindow> {
        loop {
            match self.inner.next() {
                None => break None,
                Some(Status::Timeout) => { /* Continue */ }
                Some(Status::Ok(new_window)) => break Some(new_window),
            }
        }
    }
}

impl<I> Iterator for Overlap<I>
where
    I: Iterator<Item = Status<TimeWindow>>,
{
    type Item = TimeWindow;

    fn next(&mut self) -> Option<Self::Item> {
        let old_state = std::mem::replace(&mut self.state, State::Idle);
        match old_state {
            State::Idle => {
                let new_window = self.wait_for_next_window()?;

                // Produce the first half of the new window, store the second half
                let first_half =
                    TimeWindow::new(new_window.time(), new_window.first_half().to_vec());
                self.state = State::Overlap {
                    previous_half: new_window.into_second_half(),
                };
                Some(first_half)
            }
            State::Overlap { mut previous_half } => {
                let new_window = match self.inner.next() {
                    None | Some(Status::Timeout) => {
                        // Produce previous half
                        self.state = State::Idle;
                        return Some(previous_half);
                    }
                    Some(Status::Ok(new_window)) => new_window,
                };

                // Check for a gap between the previous window and the new one
                let time_difference = new_window.time() - previous_half.time();
                match time_difference {
                    1 => {
                        // Difference 1: Overlap
                        // [--------t--------]
                        //          [--------t+1------]
                        overlap_windows(&mut previous_half, &new_window);
                        // Store the second half of new_window
                        self.state = State::Overlap {
                            previous_half: new_window.into_second_half(),
                        };
                        // Produce overlapped half-window
                        Some(previous_half)
                    }
                    2 => {
                        // Difference 2: Adjacent
                        // [--------t--------]
                        //                    [--------t+2------]

                        // Produce the previous half, followed by the first half of new_window
                        let previous_time = previous_half.time();
                        let mut samples = previous_half.into_samples();
                        samples.extend_from_slice(new_window.first_half());
                        // Store the second half of new_window
                        self.state = State::Overlap {
                            previous_half: new_window.into_second_half(),
                        };

                        Some(TimeWindow::new(previous_time, samples))
                    }
                    _ => {
                        // Difference 3: Half-window gap
                        // [--------t--------]
                        //                              [--------t+3------]
                        // Difference n: n - 2 half-window gap

                        let gap_half_windows = time_difference - 2;
                        self.state = State::Gap {
                            remaining_half_windows: gap_half_windows,
                            next_window: new_window,
                        };
                        // Produce the previous half-window
                        Some(previous_half)
                    }
                }
            }
            State::Gap {
                remaining_half_windows,
                next_window,
            } => {
                if remaining_half_windows == 0 {
                    // Produce the first half of next_window, store the rest
                    let first_half =
                        TimeWindow::new(next_window.time(), next_window.first_half().to_vec());
                    let second_half = next_window.into_second_half();
                    self.state = State::Overlap {
                        previous_half: second_half,
                    };
                    Some(first_half)
                } else {
                    // Produce a half-window, decrement
                    let half_window = TimeWindow::new(
                        next_window.time(),
                        vec![Complex32::zero(); self.window_size / 2],
                    );
                    self.state = State::Gap {
                        remaining_half_windows: remaining_half_windows - 1,
                        next_window,
                    };
                    Some(half_window)
                }
            }
        }
    }
    //     if let Some(zero_windows) = self.zero_windows {
    //         // Produce more half-length windows of zero
    //         // The window time value isn't actually used after this step, so it doesn't matter.
    //         let window_time = self.prev_window.as_ref().map(TimeWindow::time).unwrap_or(0);
    //
    //         // Decrement remaining windows to produce
    //         self.zero_windows = NonZeroU64::new(zero_windows.get() - 1);
    //         Some(TimeWindow::new(
    //             window_time,
    //             vec![Complex32::zero(); self.window_size / 2],
    //         ))
    //     } else {
    //         // Get the next window from the inner iterator
    //         loop {
    //             match self.inner.next() {
    //                 Some(Status::Ok(new_window)) => {
    //                     assert_eq!(new_window.len(), self.window_size, "Incorrect window size");
    //                     return self.handle_window(new_window);
    //                 }
    //                 Some(Status::Timeout) => {
    //                     if let Some(prev) = self.prev_window.take() {
    //                         // Send the second half of the window
    //                         return Some(prev.into_second_half());
    //                     } else {
    //                         // Continue waiting for something to happen
    //                     }
    //                 }
    //                 None => return self.handle_end(),
    //             }
    //         }
    //     }
    // }
}

#[cfg(test)]
mod test {
    use super::*;

    use num_complex::Complex32;
    use std::iter;

    #[test]
    fn test_one_window() {
        let samples = vec![
            Complex32::new(1.0, 2.0),
            Complex32::new(0.2, 0.05),
            Complex32::new(127.0, 6.21),
            Complex32::new(-0.3, -9.2),
        ];
        let windows = iter::once(TimeWindow::new(0, samples.clone()));
        check_iter(4, windows.into_iter().map(Status::Ok), &samples);
    }

    #[test]
    fn test_two_windows_no_gap() {
        let samples1 = vec![
            Complex32::new(1.0, 2.0),
            Complex32::new(0.2, 0.05),
            Complex32::new(127.0, 6.21),
            Complex32::new(-0.3, -9.2),
        ];

        let samples2 = vec![
            Complex32::new(5.0, 6.0),
            Complex32::new(3.2, 127.05),
            Complex32::new(6.0, 9.26),
            Complex32::new(-2.3, -16.2),
        ];
        // Middle two samples overlap
        let expected_samples = vec![
            Complex32::new(1.0, 2.0),
            Complex32::new(0.2, 0.05),
            Complex32::new(127.0 + 5.0, 6.21 + 6.0),
            Complex32::new(-0.3 + 3.2, -9.2 + 127.05),
            Complex32::new(6.0, 9.26),
            Complex32::new(-2.3, -16.2),
        ];

        let windows = vec![TimeWindow::new(0, samples1), TimeWindow::new(1, samples2)];
        check_iter(4, windows.into_iter().map(Status::Ok), &expected_samples);
    }

    #[test]
    fn test_two_windows_timeout() {
        let samples1 = vec![
            Complex32::new(1.0, 2.0),
            Complex32::new(0.2, 0.05),
            Complex32::new(127.0, 6.21),
            Complex32::new(-0.3, -9.2),
        ];

        let samples2 = vec![
            Complex32::new(5.0, 6.0),
            Complex32::new(3.2, 127.05),
            Complex32::new(6.0, 9.26),
            Complex32::new(-2.3, -16.2),
        ];
        // No overlap because of timeout
        let expected_samples = vec![
            Complex32::new(1.0, 2.0),
            Complex32::new(0.2, 0.05),
            Complex32::new(127.0, 6.21),
            Complex32::new(-0.3, -9.2),
            Complex32::new(5.0, 6.0),
            Complex32::new(3.2, 127.05),
            Complex32::new(6.0, 9.26),
            Complex32::new(-2.3, -16.2),
        ];

        let windows = vec![
            Status::Ok(TimeWindow::new(0, samples1)),
            Status::Timeout,
            Status::Ok(TimeWindow::new(1, samples2)),
        ];
        check_iter(4, windows, &expected_samples);
    }

    fn check_iter<I>(window_size: usize, windows: I, expected: &[Complex32])
    where
        I: IntoIterator<Item = Status<TimeWindow>>,
    {
        let overlap = Overlap::new(windows.into_iter(), window_size);
        let result = overlap.flatten().collect::<Vec<Complex32>>();
        assert_eq!(&*result, expected);
    }

    #[test]
    fn gap_adjacent() {
        // Windows with a time difference of 2 get concatenated together without a gap
        let windows = [
            TimeWindow::new(1, vec![Complex32::new(1.0, 1.0), Complex32::new(0.0, 2.0)]),
            TimeWindow::new(3, vec![Complex32::new(7.0, 8.0), Complex32::new(9.0, 10.0)]),
        ];
        let expected = [
            Complex32::new(1.0, 1.0),
            Complex32::new(0.0, 2.0),
            Complex32::new(7.0, 8.0),
            Complex32::new(9.0, 10.0),
        ];
        check_iter(2, windows.iter().cloned().map(Status::Ok), &expected);
    }

    #[test]
    fn gap_one_half() {
        // Windows with a time difference of 3 are separated by a half-window gap
        let windows = [
            TimeWindow::new(1, vec![Complex32::new(1.0, 1.0), Complex32::new(0.0, 2.0)]),
            TimeWindow::new(4, vec![Complex32::new(7.0, 8.0), Complex32::new(9.0, 10.0)]),
        ];
        let expected = [
            Complex32::new(1.0, 1.0),
            Complex32::new(0.0, 2.0),
            Complex32::zero(), // gap
            Complex32::new(7.0, 8.0),
            Complex32::new(9.0, 10.0),
        ];
        check_iter(2, windows.iter().cloned().map(Status::Ok), &expected);
    }

    #[test]
    fn gap_two_half() {
        // Windows with a time difference of 4 are separated by a full-window gap
        let windows = [
            TimeWindow::new(1, vec![Complex32::new(1.0, 1.0), Complex32::new(0.0, 2.0)]),
            TimeWindow::new(5, vec![Complex32::new(7.0, 8.0), Complex32::new(9.0, 10.0)]),
        ];
        let expected = [
            Complex32::new(1.0, 1.0),
            Complex32::new(0.0, 2.0),
            Complex32::zero(), // gap
            Complex32::zero(), // gap
            Complex32::new(7.0, 8.0),
            Complex32::new(9.0, 10.0),
        ];
        check_iter(2, windows.iter().cloned().map(Status::Ok), &expected);
    }

    #[test]
    fn gap_three_half() {
        // Windows with a time difference of 5 are separated by a 1.5-window gap
        let windows = [
            TimeWindow::new(1, vec![Complex32::new(1.0, 1.0), Complex32::new(0.0, 2.0)]),
            TimeWindow::new(6, vec![Complex32::new(7.0, 8.0), Complex32::new(9.0, 10.0)]),
        ];
        let expected = [
            Complex32::new(1.0, 1.0),
            Complex32::new(0.0, 2.0),
            Complex32::zero(), // gap
            Complex32::zero(), // gap
            Complex32::zero(), // gap
            Complex32::new(7.0, 8.0),
            Complex32::new(9.0, 10.0),
        ];
        check_iter(2, windows.iter().cloned().map(Status::Ok), &expected);
    }
}
