use std::time::{Duration, Instant};

pub(crate) const MANUAL_PAUSE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct Motion {
    from: f32,
    to: f32,
    started: Option<Instant>,
    duration: Duration,
    next_allowed: Option<Instant>,
    first_done: bool,
    index: Option<usize>,
    pending: Option<(usize, f32)>,
}
impl Motion {
    fn sample(&self, now: Instant) -> (f32, bool) {
        let Some(started) = self.started else {
            return (self.to, false);
        };
        let progress = (now.saturating_duration_since(started).as_secs_f32()
            / self.duration.as_secs_f32())
        .min(1.);
        let eased = 1. - (1. - progress).powi(3);
        (self.from + (self.to - self.from) * eased, progress < 1.)
    }
    fn animate(&mut self, from: f32, to: f32, smooth: bool, now: Instant) {
        let first = from == 0. && !self.first_done;
        self.first_done |= first;
        self.from = from;
        self.to = to;
        if smooth && !first && (to - from).abs() > 2. {
            self.duration =
                Duration::from_secs_f32(((to - from).abs() * 0.55).clamp(350., 850.) / 1000.);
            self.started = Some(now);
            self.next_allowed = Some(now + self.duration + Duration::from_millis(20));
        } else {
            self.duration = Duration::ZERO;
            self.started = None;
            self.next_allowed = None;
        }
    }
    pub(crate) fn request(&mut self, index: usize, target: f32, current: f32, now: Instant) {
        if self.index == Some(index) && (target - self.to).abs() < 0.5 {
            return;
        }
        self.index = Some(index);
        if self.next_allowed.is_some_and(|until| now <= until) {
            self.pending = Some((index, target));
        } else {
            self.pending = None;
            self.animate(current, target, true, now);
        }
    }
    pub(crate) fn active(&self) -> bool {
        self.started.is_some() || self.pending.is_some()
    }
    pub(crate) fn tick(&mut self, now: Instant) -> (f32, bool) {
        let (current, animating) = self.sample(now);
        if self.pending.is_some() && self.next_allowed.is_none_or(|until| now > until) {
            let (index, target) = self.pending.take().unwrap();
            self.index = Some(index);
            self.animate(current, target, true, now);
            return self.sample(now);
        }
        if !animating && self.pending.is_none() {
            self.started = None;
        }
        (current, animating || self.pending.is_some())
    }
    pub(crate) fn cancel(&mut self, current: f32) {
        self.from = current;
        self.to = current;
        self.started = None;
        self.next_allowed = None;
        self.pending = None;
        self.index = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lyric_scroll_matches_original_trajectories_and_queued_line_changes() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyrics-scroll-reference.json")).unwrap();
        assert_eq!(
            MANUAL_PAUSE.as_millis(),
            reference["manualPauseMs"].as_u64().unwrap() as u128
        );
        for case in reference["ratios"].as_array().unwrap() {
            assert!(
                (feishin_core::lyrics::follow_ratio(case["alignment"].as_f64().unwrap() as f32)
                    - case["ratio"].as_f64().unwrap() as f32)
                    .abs()
                    < 0.00001
            );
        }
        let now = Instant::now();
        for case in reference["cases"].as_array().unwrap() {
            let mut motion = Motion {
                first_done: case["first"] != true,
                ..Default::default()
            };
            motion.animate(
                case["start"].as_f64().unwrap() as f32,
                case["target"].as_f64().unwrap() as f32,
                case["smooth"] == true,
                now,
            );
            assert!(
                (motion.duration.as_secs_f32() * 1000. - case["duration"].as_f64().unwrap() as f32)
                    .abs()
                    < 0.001,
                "{case}"
            );
            for frame in case["positions"].as_array().unwrap() {
                let (value, _) =
                    motion.tick(now + Duration::from_millis(frame["elapsed"].as_u64().unwrap()));
                assert!(
                    (value - frame["value"].as_f64().unwrap() as f32).abs() < 0.001,
                    "{case}: {frame}, actual={value}"
                );
            }
        }
        let mut motion = Motion::default();
        motion.request(0, -100., 0., now);
        assert_eq!(motion.tick(now), (-100., false));
        motion.request(1, -200., -100., now);
        let (value, active) = motion.tick(now + Duration::from_millis(175));
        assert!(active && (value + 187.5).abs() < 0.001);
        motion.request(2, -500., value, now + Duration::from_millis(175));
        assert_eq!(motion.tick(now + Duration::from_millis(350)), (-200., true));
        assert_eq!(motion.tick(now + Duration::from_millis(371)), (-200., true));
        assert_eq!(
            motion.tick(now + Duration::from_millis(721)),
            (-500., false)
        );
        motion.cancel(-350.);
        assert_eq!(motion.tick(now + Duration::from_secs(3)), (-350., false));
    }
}
