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
