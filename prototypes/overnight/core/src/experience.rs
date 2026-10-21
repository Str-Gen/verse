//! Timing policy for the accepted preparation and gradual speed buildup.
use serde::Serialize;

pub const PREPARATION_MS: u64 = 2_000;
pub const RAMP_MS: u64 = 30_000;
pub const BRIEF_PAUSE_MS: u64 = 30_000;

#[derive(Default)]
pub struct Comfort {
    started: Option<u64>,
    paused: Option<u64>,
    began: bool,
}
impl Comfort {
    pub fn start(&mut self, now: u64) {
        let brief = self.began
            && self
                .paused
                .is_some_and(|at| now.saturating_sub(at) <= BRIEF_PAUSE_MS);
        self.started = if brief { None } else { Some(now) };
        if !brief {
            self.began = false;
        }
        self.paused = None;
    }
    pub fn pause(&mut self, now: u64) {
        self.began |= self
            .started
            .is_some_and(|at| now.saturating_sub(at) >= PREPARATION_MS);
        self.started = None;
        self.paused = Some(now);
    }
    pub fn preparation_left(&self, now: u64) -> u64 {
        self.started.map_or(0, |at| {
            PREPARATION_MS.saturating_sub(now.saturating_sub(at))
        })
    }
    pub fn fraction(&self, now: u64) -> f64 {
        self.started.map_or(1.0, |at| {
            ramp(now.saturating_sub(at).saturating_sub(PREPARATION_MS))
        })
    }
}
pub fn ramp(elapsed: u64) -> f64 {
    let progress = (elapsed as f64 / RAMP_MS as f64).clamp(0.0, 1.0);
    0.5 + 0.25 * (1.0 + ((progress * 2.0 - 1.0) * 2.0).tanh() / 2.0_f64.tanh())
}
#[derive(Clone, Serialize)]
pub struct Exposure {
    pub piece: usize,
    pub start: u64,
    pub end: u64,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_cancel_brief_resume_and_long_pause() {
        let mut comfort = Comfort::default();
        comfort.start(0);
        assert_eq!(comfort.preparation_left(500), 1500);
        comfort.pause(500);
        comfort.start(600);
        assert_eq!(comfort.preparation_left(600), 2000);
        assert!((comfort.fraction(2600) - 0.5).abs() < 1e-9);
        assert!((comfort.fraction(17600) - 0.75).abs() < 1e-9);
        assert!((comfort.fraction(32600) - 1.0).abs() < 1e-9);
        comfort.pause(33000);
        comfort.start(34000);
        assert_eq!(comfort.preparation_left(34000), 0);
        assert_eq!(comfort.fraction(34000), 1.0);
        comfort.pause(35000);
        comfort.start(66000);
        assert_eq!(comfort.preparation_left(66000), 2000);
    }
}
