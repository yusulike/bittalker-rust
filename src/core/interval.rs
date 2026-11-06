//! Interval boundary-crossing tracker.
//!
//! - "돌파" (Up): price crosses above/at a boundary (e.g. 94999.9 -> 95000.1)
//! - "깨어짐" (Down): price crosses below a boundary (e.g. 95000.1 -> 94999.9)
//!
//! Boundaries are exact integer multiples of `interval`.
//! Every boundary between the previous and current price is reported in
//! chronological crossing order. Zero allocations, O(1) integer arithmetic.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Up => "UP",
            Direction::Down => "DOWN",
        }
    }
}

pub struct IntervalTracker {
    pub interval: i64,
    pub last_price: Option<f64>,
}

impl IntervalTracker {
    pub fn new(interval: i64) -> Self {
        Self {
            interval,
            last_price: None,
        }
    }

    /// Update interval setting.
    pub fn set_interval(&mut self, interval: i64) {
        self.interval = interval;
    }

    /// Process a new price and return `(boundary, direction)` crossings in
    /// chronological crossing order.
    pub fn process_price(&mut self, current_price: f64) -> Vec<(i64, Direction)> {
        let Some(last) = self.last_price else {
            self.last_price = Some(current_price);
            return Vec::new();
        };
        self.last_price = Some(current_price);

        if self.interval <= 0 {
            return Vec::new();
        }

        let prev = last.floor() as i64;
        let curr = current_price.floor() as i64;
        let mut crossings = Vec::new();

        if curr > prev {
            let start = prev / self.interval + 1;
            let end = curr / self.interval;
            for k in start..=end {
                crossings.push((k * self.interval, Direction::Up));
            }
        } else if curr < prev {
            let start = prev / self.interval;
            let end = curr / self.interval + 1;
            for k in (end..=start).rev() {
                crossings.push((k * self.interval, Direction::Down));
            }
        }

        crossings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(t: &mut IntervalTracker, price: f64) -> Vec<(i64, &'static str)> {
        t.process_price(price)
            .into_iter()
            .map(|(p, d)| (p, d.as_str()))
            .collect()
    }

    #[test]
    fn test_cross_up() {
        let mut t = IntervalTracker::new(50);
        assert!(events(&mut t, 9440.0).is_empty()); // Init
        assert_eq!(events(&mut t, 9455.0), vec![(9450, "UP")]);
    }

    #[test]
    fn test_cross_down() {
        let mut t = IntervalTracker::new(50);
        assert!(events(&mut t, 9460.0).is_empty()); // Init
        assert_eq!(events(&mut t, 9440.0), vec![(9450, "DOWN")]);
    }

    #[test]
    fn test_multiple_cross() {
        let mut t = IntervalTracker::new(50);
        assert!(events(&mut t, 9400.0).is_empty()); // Init
        assert_eq!(
            events(&mut t, 9510.0),
            vec![(9450, "UP"), (9500, "UP")]
        );
    }

    #[test]
    fn test_oscillation_reports_only_first_crossing_each_way() {
        let mut t = IntervalTracker::new(50);
        assert!(events(&mut t, 9449.0).is_empty());
        assert_eq!(events(&mut t, 9451.0), vec![(9450, "UP")]);
        // Hover above the same boundary again: no repeat
        assert!(events(&mut t, 9450.5).is_empty());
        assert!(events(&mut t, 9451.0).is_empty());
        assert_eq!(events(&mut t, 9449.0), vec![(9450, "DOWN")]);
        // and back up again re-triggers
        assert_eq!(events(&mut t, 9451.0), vec![(9450, "UP")]);
    }

    #[test]
    fn test_multi_interval_jump_down_reports_in_order() {
        let mut t = IntervalTracker::new(50);
        assert!(events(&mut t, 9520.0).is_empty());
        assert_eq!(
            events(&mut t, 9410.0),
            vec![(9500, "DOWN"), (9450, "DOWN")]
        );
    }

    #[test]
    fn set_interval_resets_state() {
        let mut t = IntervalTracker::new(50);
        assert!(events(&mut t, 9449.0).is_empty());
        t.set_interval(100);
        // After reset, states are re-inferred: no crossing is falsely reported
        assert!(events(&mut t, 9451.0).is_empty());
    }
}
