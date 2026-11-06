//! Interval boundary-crossing tracker.
//! Direct port of `src/core/interval_logic.py` (without the Qt signal layer):
//! `process_price` returns the crossings that occurred, in chronological order.
//!
//! - "돌파" (Up): price crosses above a boundary (e.g. 94999.9 -> 95000.1)
//! - "깨어짐" (Down): price crosses below a boundary (e.g. 95000.1 -> 94999.9)
//!
//! Every boundary between the previous and current price is checked, so a
//! single tick that jumps multiple intervals still reports each crossing.
//! Only reports when the state CHANGES; if price oscillates around a boundary,
//! only the first crossing in each direction triggers.

use std::collections::HashMap;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Above,
    Below,
}

/// Number of boundaries on each side of the current price whose state is kept
pub const KEEP_INTERVALS: i64 = 2;

pub struct IntervalTracker {
    pub interval: f64,
    pub last_price: Option<f64>,
    /// Last notified state for each tracked boundary ("ABOVE" / "BELOW")
    boundary_states: HashMap<i64, Side>,
}

impl IntervalTracker {
    pub fn new(interval: f64) -> Self {
        Self {
            interval,
            last_price: None,
            boundary_states: HashMap::new(),
        }
    }

    /// Clear state when interval changes; states are re-inferred from the
    /// next price so no crossing is falsely reported or missed.
    pub fn set_interval(&mut self, interval: f64) {
        self.interval = interval;
        self.boundary_states.clear();
    }

    /// Process a new price and return `(boundary, direction)` crossings in
    /// chronological crossing order. Boundaries are multiples of `interval`.
    pub fn process_price(&mut self, current_price: f64) -> Vec<(f64, Direction)> {
        let Some(last) = self.last_price else {
            self.last_price = Some(current_price);
            return Vec::new();
        };
        // Check every boundary between the previous and current price so that
        // gaps larger than one interval don't skip crossings.
        let lower = last.min(current_price);
        let upper = last.max(current_price);
        let first_index = (lower / self.interval).floor() as i64;
        let last_index = (upper / self.interval).floor() as i64;

        let mut crossings = Vec::new();
        let mut indices: Vec<i64> = (first_index..=last_index).collect();
        if current_price < last {
            indices.reverse(); // emit in chronological crossing order
        }
        for i in indices {
            let boundary = i as f64 * self.interval;
            if let Some(dir) = self.check_boundary(current_price, last, boundary) {
                crossings.push((boundary, dir));
            }
        }

        self.last_price = Some(current_price);
        self.prune_far_boundaries(current_price);
        crossings
    }

    /// Determine if price is above or below the boundary.
    /// Exactly on the boundary counts as "equal" and never triggers.
    fn get_state(price: f64, boundary: f64) -> Option<Side> {
        if price > boundary {
            Some(Side::Above)
        } else if price < boundary {
            Some(Side::Below)
        } else {
            None
        }
    }

    fn check_boundary(&mut self, current_price: f64, last_price: f64, boundary: f64) -> Option<Direction> {
        let current_state = Self::get_state(current_price, boundary)?;

        // Boundary key: the integer interval index (mirrors the exact float
        // boundary value used by the original for repeated ticks).
        let key = (boundary / self.interval).round() as i64;

        let last_state = match self.boundary_states.get(&key) {
            Some(s) => Some(*s),
            None => {
                // First time seeing this boundary: infer its previous side from
                // the previous price instead of silently adopting the current one
                Self::get_state(last_price, boundary)
            }
        };

        let crossing = match (current_state, last_state) {
            (Side::Above, Some(Side::Below)) => Some(Direction::Up),
            (Side::Below, Some(Side::Above)) => Some(Direction::Down),
            _ => None,
        };

        self.boundary_states.insert(key, current_state);
        crossing
    }

    /// Drop tracked boundaries far from the current price to bound memory.
    fn prune_far_boundaries(&mut self, price: f64) {
        let center = (price / self.interval).round() as i64;
        let low = (center - KEEP_INTERVALS) as f64 * self.interval;
        let high = (center + KEEP_INTERVALS) as f64 * self.interval;
        self.boundary_states.retain(|key, _| {
            let b = *key as f64 * self.interval;
            b >= low && b <= high
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(t: &mut IntervalTracker, price: f64) -> Vec<(f64, &'static str)> {
        t.process_price(price)
            .into_iter()
            .map(|(p, d)| (p, d.as_str()))
            .collect()
    }

    #[test]
    fn test_cross_up() {
        let mut t = IntervalTracker::new(50.0);
        assert!(events(&mut t, 9440.0).is_empty()); // Init
        assert_eq!(events(&mut t, 9455.0), vec![(9450.0, "UP")]);
    }

    #[test]
    fn test_cross_down() {
        let mut t = IntervalTracker::new(50.0);
        assert!(events(&mut t, 9460.0).is_empty()); // Init
        assert_eq!(events(&mut t, 9440.0), vec![(9450.0, "DOWN")]);
    }

    #[test]
    fn test_multiple_cross() {
        let mut t = IntervalTracker::new(50.0);
        assert!(events(&mut t, 9400.0).is_empty()); // Init
        assert_eq!(
            events(&mut t, 9510.0),
            vec![(9450.0, "UP"), (9500.0, "UP")]
        );
    }

    #[test]
    fn test_oscillation_reports_only_first_crossing_each_way() {
        let mut t = IntervalTracker::new(50.0);
        assert!(events(&mut t, 9449.0).is_empty());
        assert_eq!(events(&mut t, 9451.0), vec![(9450.0, "UP")]);
        // Hover above the same boundary again: no repeat
        assert!(events(&mut t, 9450.5).is_empty());
        assert!(events(&mut t, 9451.0).is_empty());
        assert_eq!(events(&mut t, 9449.0), vec![(9450.0, "DOWN")]);
        // and back up again re-triggers
        assert_eq!(events(&mut t, 9451.0), vec![(9450.0, "UP")]);
    }

    #[test]
    fn test_multi_interval_jump_down_reports_in_order() {
        let mut t = IntervalTracker::new(50.0);
        assert!(events(&mut t, 9520.0).is_empty());
        assert_eq!(
            events(&mut t, 9410.0),
            vec![(9500.0, "DOWN"), (9450.0, "DOWN")]
        );
    }

    #[test]
    fn set_interval_resets_state() {
        let mut t = IntervalTracker::new(50.0);
        assert!(events(&mut t, 9449.0).is_empty());
        t.set_interval(100.0);
        // After reset, states are re-inferred: no crossing is falsely reported
        assert!(events(&mut t, 9451.0).is_empty());
    }
}
