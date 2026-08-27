use serde::{Deserialize, Serialize};

pub const TICKS_PER_MINUTE: u64 = 60;
pub const TICKS_PER_HOUR: u64 = 3_600;
pub const TICKS_PER_DAY: u64 = 86_400;
pub const DAYS_PER_MONTH: u64 = 30;
pub const MONTHS_PER_YEAR: u64 = 12;
pub const DAYS_PER_YEAR: u64 = DAYS_PER_MONTH * MONTHS_PER_YEAR; // 360-day calendar
pub const TICKS_PER_MONTH: u64 = TICKS_PER_DAY * DAYS_PER_MONTH;
pub const TICKS_PER_YEAR: u64 = TICKS_PER_DAY * DAYS_PER_YEAR;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub struct SimTick(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub struct SimDuration(pub u64);

impl SimTick {
    #[inline]
    pub fn advance(&self, duration: SimDuration) -> Self {
        Self(self.0.saturating_add(duration.0))
    }

    #[inline]
    pub fn delta_from(&self, earlier: SimTick) -> SimDuration {
        SimDuration(self.0.saturating_sub(earlier.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SimClock {
    pub current_tick: SimTick,
    pub speed_multiplier: f32,
    pub is_paused: bool,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            current_tick: SimTick(0),
            speed_multiplier: 1.0,
            is_paused: false,
        }
    }
}

impl SimClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn step(&mut self, ticks: u64) -> SimTick {
        if !self.is_paused {
            let actual_ticks = (ticks as f32 * self.speed_multiplier).round().max(1.0) as u64;
            self.current_tick.0 = self.current_tick.0.saturating_add(actual_ticks);
        }
        self.current_tick
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimCalendar {
    pub year: u32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl SimCalendar {
    pub fn from_tick(tick: SimTick) -> Self {
        let total_seconds = tick.0;
        let second = (total_seconds % 60) as u8;
        let total_minutes = total_seconds / 60;
        let minute = (total_minutes % 60) as u8;
        let total_hours = total_minutes / 60;
        let hour = (total_hours % 24) as u8;
        let total_days = total_hours / 24;

        let day = (total_days % DAYS_PER_MONTH) as u8 + 1;
        let total_months = total_days / DAYS_PER_MONTH;
        let month = (total_months % MONTHS_PER_YEAR) as u8 + 1;
        let year = (total_months / MONTHS_PER_YEAR) as u32 + 1;

        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_constants_are_consistent() {
        assert_eq!(TICKS_PER_HOUR, TICKS_PER_MINUTE * 60);
        assert_eq!(TICKS_PER_DAY, TICKS_PER_HOUR * 24);
        assert_eq!(DAYS_PER_YEAR, 360);
        assert_eq!(TICKS_PER_MONTH, TICKS_PER_DAY * DAYS_PER_MONTH);
        assert_eq!(TICKS_PER_YEAR, TICKS_PER_DAY * DAYS_PER_YEAR);
    }

    #[test]
    fn tick_advance_saturates_instead_of_overflowing() {
        assert_eq!(SimTick(10).advance(SimDuration(5)), SimTick(15));
        assert_eq!(SimTick(u64::MAX).advance(SimDuration(9)), SimTick(u64::MAX));
    }

    #[test]
    fn tick_delta_saturates_for_future_reference() {
        assert_eq!(SimTick(100).delta_from(SimTick(40)), SimDuration(60));
        assert_eq!(SimTick(40).delta_from(SimTick(100)), SimDuration(0));
    }

    #[test]
    fn clock_default_starts_at_zero_and_unpaused() {
        let clock = SimClock::new();
        assert_eq!(clock.current_tick, SimTick(0));
        assert_eq!(clock.speed_multiplier, 1.0);
        assert!(!clock.is_paused);
    }

    #[test]
    fn clock_step_scales_by_speed_multiplier() {
        let mut clock = SimClock::new();
        clock.speed_multiplier = 3.0;
        assert_eq!(clock.step(100), SimTick(300));
        assert_eq!(clock.step(100), SimTick(600));
    }

    #[test]
    fn clock_step_always_advances_at_least_one_tick() {
        let mut clock = SimClock::new();
        clock.speed_multiplier = 0.0;
        assert_eq!(clock.step(1000), SimTick(1));
    }

    #[test]
    fn paused_clock_does_not_advance() {
        let mut clock = SimClock::new();
        clock.is_paused = true;
        assert_eq!(clock.step(TICKS_PER_DAY), SimTick(0));
    }

    #[test]
    fn calendar_from_tick_zero_is_first_day_of_first_year() {
        let cal = SimCalendar::from_tick(SimTick(0));
        assert_eq!((cal.year, cal.month, cal.day), (1, 1, 1));
        assert_eq!((cal.hour, cal.minute, cal.second), (0, 0, 0));
    }

    #[test]
    fn calendar_from_tick_decomposes_time_of_day() {
        let tick = SimTick(TICKS_PER_HOUR * 13 + TICKS_PER_MINUTE * 45 + 30);
        let cal = SimCalendar::from_tick(tick);
        assert_eq!((cal.hour, cal.minute, cal.second), (13, 45, 30));
        assert_eq!((cal.year, cal.month, cal.day), (1, 1, 1));
    }

    #[test]
    fn calendar_rolls_over_days_months_and_years() {
        let cal = SimCalendar::from_tick(SimTick(TICKS_PER_DAY * 30));
        assert_eq!((cal.year, cal.month, cal.day), (1, 2, 1));

        let cal = SimCalendar::from_tick(SimTick(TICKS_PER_YEAR));
        assert_eq!((cal.year, cal.month, cal.day), (2, 1, 1));

        let cal = SimCalendar::from_tick(SimTick(
            TICKS_PER_YEAR * 7 + TICKS_PER_MONTH * 3 + TICKS_PER_DAY * 4,
        ));
        assert_eq!((cal.year, cal.month, cal.day), (8, 4, 5));
    }
}
