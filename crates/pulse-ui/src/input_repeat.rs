//! Repeat of *directional* joystick GPIO press edges only.
//! Physical center click and four page keys are deliberately never repeated.
//! Caller delivers fresh press edges separately; only delayed repeats come
//! from this state machine. No delayed tasks, allocation or hardware writes.

const DIRECTIONS: [usize; 4] = [4, 5, 7, 8];
const INITIAL_DELAY_MS: u64 = 440;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputRepeater {
    started_at: [Option<u64>; 9],
    next_at: [u64; 9],
}

impl Default for InputRepeater {
    fn default() -> Self {
        Self::new()
    }
}

impl InputRepeater {
    pub const fn new() -> Self {
        Self {
            started_at: [None; 9],
            next_at: [0; 9],
        }
    }

    pub fn reset(&mut self) {
        self.started_at = [None; 9];
        self.next_at = [0; 9];
    }

    /// Returns a raw-key bitmask for repeat events. A brand-new press
    /// returns zero; the normal input edge detector handles it immediately.
    /// At most one repeat per direction is emitted per sample.
    pub fn poll(&mut self, held_mask: u16, now_ms: u64) -> u16 {
        let mut repeat_mask = 0u16;
        for index in DIRECTIONS {
            if held_mask & (1u16 << index) == 0 {
                self.started_at[index] = None;
                self.next_at[index] = 0;
                continue;
            }
            match self.started_at[index] {
                None => {
                    self.started_at[index] = Some(now_ms);
                    self.next_at[index] = now_ms.saturating_add(INITIAL_DELAY_MS);
                }
                Some(started) => {
                    if self.next_at[index] != u64::MAX && now_ms >= self.next_at[index] {
                        repeat_mask |= 1u16 << index;
                        let elapsed = now_ms.saturating_sub(started);
                        let period = if elapsed >= 3000 {
                            100
                        } else if elapsed >= 1600 {
                            155
                        } else {
                            240
                        };
                        self.next_at[index] = now_ms.saturating_add(period);
                    }
                }
            }
        }
        repeat_mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_press_is_immediate_only_in_caller_then_delayed_repeat() {
        let mut x = InputRepeater::new();
        let down = 1 << 5;
        assert_eq!(x.poll(down, 100), 0);
        assert_eq!(x.poll(down, 539), 0);
        assert_eq!(x.poll(down, 540), down);
        assert_eq!(x.poll(down, 600), 0);
        assert_eq!(x.poll(down, 780), down);
    }

    #[test]
    fn repeating_accelerates_and_remains_bounded() {
        let mut x = InputRepeater::new();
        let up = 1 << 7;
        x.poll(up, 0);
        assert_eq!(x.poll(up, 440), up);
        assert_eq!(x.poll(up, 680), up);
        assert_eq!(x.poll(up, 1610), up);
        assert_eq!(x.poll(up, 1764), 0);
        assert_eq!(x.poll(up, 1765), up);
        assert_eq!(x.poll(up, 3000), up);
        assert_eq!(x.poll(up, 3099), 0);
        assert_eq!(x.poll(up, 3100), up);
    }

    #[test]
    fn release_restarts_delay_and_reset_blocks_held_repeats() {
        let mut x = InputRepeater::new();
        let left = 1 << 4;
        x.poll(left, 0);
        assert_eq!(x.poll(left, 440), left);
        assert_eq!(x.poll(0, 500), 0);
        assert_eq!(x.poll(left, 530), 0);
        assert_eq!(x.poll(left, 900), 0);
        assert_eq!(x.poll(left, 970), left);
        x.reset();
        assert_eq!(x.poll(left, 5000), 0);
    }

    #[test]
    fn click_and_four_page_buttons_are_never_repeated() {
        let mut x = InputRepeater::new();
        let mask = 0b1111 | (1 << 6);
        x.poll(mask, 0);
        assert_eq!(x.poll(mask, 10_000), 0);
    }

    #[test]
    fn can_repeat_two_distinct_directional_inputs_without_overflow() {
        let mut x = InputRepeater::new();
        let mask = (1 << 4) | (1 << 8);
        x.poll(mask, u64::MAX - 500);
        assert_eq!(x.poll(mask, u64::MAX - 60), mask);
        assert_eq!(x.poll(mask, u64::MAX), 0);
    }
}
