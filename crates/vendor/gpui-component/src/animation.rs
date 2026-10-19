/// A cubic bezier function like CSS `cubic-bezier`.
///
/// Builder:
///
/// https://cubic-bezier.com
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 {
    move |t: f32| {
        let one_t = 1.0 - t;
        let one_t2 = one_t * one_t;
        let t2 = t * t;
        let t3 = t2 * t;

        // The Bezier curve function for x and y, where x0 = 0, y0 = 0, x3 = 1, y3 = 1
        let _x = 3.0 * x1 * one_t2 * t + 3.0 * x2 * one_t * t2 + t3;
        let y = 3.0 * y1 * one_t2 * t + 3.0 * y2 * one_t * t2 + t3;

        y
    }
}

/// A value that can be interpolated by the shared CSS transition clock.
pub trait CssValue: Copy + PartialEq {
    fn interpolate(self, target: Self, progress: f32) -> Self;
}
impl CssValue for f32 {
    fn interpolate(self, target: Self, progress: f32) -> Self {
        self + (target - self) * progress
    }
}
impl<const N: usize> CssValue for [f32; N] {
    fn interpolate(self, target: Self, progress: f32) -> Self {
        std::array::from_fn(|index| self[index].interpolate(target[index], progress))
    }
}

/// A CSS transition with timing curves and shortened interrupted reversals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssTiming {
    pub duration: std::time::Duration,
    pub delay: f32,
    pub curve: [f32; 4],
    /// Number of intervals, jump at the start, jump at the end.
    pub steps: Option<(u32, bool, bool)>,
}

pub struct CssTransition<T: CssValue = f32> {
    from: T,
    target: T,
    started: Option<std::time::Instant>,
    reversing_start: T,
    shortening: f32,
    duration: std::time::Duration,
    curve: [f32; 4],
    delay: f32,
    steps: Option<(u32, bool, bool)>,
}
impl<T: CssValue> CssTransition<T> {
    /// Start at a settled value with a duration and CSS cubic-bezier control points.
    pub fn new(value: T, duration: std::time::Duration, curve: [f32; 4]) -> Self {
        Self {
            from: value,
            target: value,
            started: None,
            reversing_start: value,
            shortening: 1.,
            duration,
            curve,
            delay: 0.,
            steps: None,
        }
    }
    fn ease(&self, progress: f32) -> f32 {
        if let Some((count, start, end)) = self.steps {
            // CSS Easing 1, section 2.3.1. The before-delay phase is handled by value().
            let jumps = count as f32 - 1. + f32::from(start) + f32::from(end);
            return ((progress * count as f32).floor() + f32::from(start)).clamp(0., jumps) / jumps;
        }
        if progress <= 0. || progress >= 1. {
            return progress;
        }
        let [x1, y1, x2, y2] = self.curve;
        let mut low = 0.;
        let mut high = 1.;
        for _ in 0..24 {
            let t = (low + high) / 2.;
            let x = 3. * (1. - t) * (1. - t) * t * x1 + 3. * (1. - t) * t * t * x2 + t * t * t;
            if x < progress { low = t } else { high = t }
        }
        let t = (low + high) / 2.;
        3. * (1. - t) * (1. - t) * t * y1 + 3. * (1. - t) * t * t * y2 + t * t * t
    }
    fn progress(&self, now: std::time::Instant) -> f32 {
        let Some(started) = self.started else {
            return 1.;
        };
        let delay = if self.delay < 0. {
            self.delay * self.shortening
        } else {
            self.delay
        };
        let elapsed = now.saturating_duration_since(started).as_secs_f32() - delay;
        if elapsed < 0. {
            return -1.;
        }
        if self.duration.is_zero() || self.shortening == 0. {
            return 1.;
        }
        (elapsed / (self.duration.as_secs_f32() * self.shortening)).clamp(0., 1.)
    }
    /// Return the current value and whether another animation frame is needed.
    pub fn value(&self, now: std::time::Instant) -> (T, bool) {
        let progress = self.progress(now);
        if progress < 0. {
            return (self.from, true);
        }
        if progress >= 1. {
            return (self.target, false);
        }
        (
            self.from.interpolate(self.target, self.ease(progress)),
            true,
        )
    }
    /// Return the destination, including while a transition is running.
    pub fn target(&self) -> T {
        self.target
    }
    /// Start or reverse a transition from its current interpolated value.
    pub fn retarget(&mut self, target: T, now: std::time::Instant) {
        if self.target == target {
            return;
        }
        let (value, animating) = self.value(now);
        if value == target {
            self.from = target;
            self.target = target;
            self.started = None;
            return;
        }
        // CSS Transitions 1, section 3: shorten interrupted reversals.
        if animating && self.reversing_start == target {
            self.shortening = (self.ease(self.progress(now).max(0.)) * self.shortening + 1.
                - self.shortening)
                .abs()
                .clamp(0., 1.);
            self.reversing_start = self.target;
        } else {
            self.shortening = 1.;
            self.reversing_start = value;
        }
        self.from = value;
        self.target = target;
        self.started = Some(now);
    }

    /// Retarget after a CSS rule changes its timing, preserving interrupted reversal shortening.
    pub fn retarget_with_timing(
        &mut self,
        target: T,
        duration: std::time::Duration,
        curve: [f32; 4],
        now: std::time::Instant,
    ) {
        self.retarget_with_options(
            target,
            CssTiming {
                duration,
                delay: 0.,
                curve,
                steps: None,
            },
            now,
        );
    }

    pub fn retarget_with_options(&mut self, target: T, timing: CssTiming, now: std::time::Instant) {
        if timing.duration.as_secs_f32() + timing.delay <= 0. {
            self.from = target;
            self.target = target;
            self.started = None;
            return;
        }
        if self.target == target {
            return;
        }
        self.retarget(target, now);
        self.duration = timing.duration;
        self.delay = timing.delay;
        self.curve = timing.curve;
        self.steps = timing.steps;
    }
}

/// Premultiplied color transition in the browser's interpolation space.
pub struct CssColorTransition {
    values: CssTransition<[f32; 4]>,
    target: gpui::Hsla,
    space: gpui::ColorSpace,
}
impl CssColorTransition {
    pub fn new(
        color: gpui::Hsla,
        space: gpui::ColorSpace,
        duration: std::time::Duration,
        curve: [f32; 4],
    ) -> Self {
        Self {
            values: CssTransition::new(Self::components(color, space), duration, curve),
            target: color,
            space,
        }
    }
    pub fn retarget(&mut self, color: gpui::Hsla, now: std::time::Instant) {
        self.target = color;
        self.values
            .retarget(Self::components(color, self.space), now);
    }
    pub fn retarget_with_options(
        &mut self,
        color: gpui::Hsla,
        space: gpui::ColorSpace,
        timing: CssTiming,
        now: std::time::Instant,
    ) {
        if self.space != space {
            // Preserve the in-flight value and reversal history when one endpoint
            // changes from a legacy RGB color to a modern interpolation space.
            if !self.values.value(now).1 {
                self.values = CssTransition::new(
                    Self::components(self.target, space),
                    timing.duration,
                    timing.curve,
                );
            } else {
                for value in [
                    &mut self.values.from,
                    &mut self.values.target,
                    &mut self.values.reversing_start,
                ] {
                    *value = Self::components(Self::color(*value, self.space), space);
                }
            }
            self.space = space;
        }
        self.target = color;
        self.values
            .retarget_with_options(Self::components(color, space), timing, now);
    }
    pub fn value(&self, now: std::time::Instant) -> (gpui::Hsla, bool) {
        let (value, animating) = self.values.value(now);
        if !animating {
            return (self.target, false);
        }
        (Self::color(value, self.space), true)
    }
    fn color(value: [f32; 4], space: gpui::ColorSpace) -> gpui::Hsla {
        let alpha = value[3];
        if alpha <= 0. {
            return gpui::transparent_black();
        }
        let [x, y, z] = std::array::from_fn(|index| value[index] / alpha);
        let [r, g, b] = match space {
            gpui::ColorSpace::Srgb => [x, y, z],
            gpui::ColorSpace::Oklab => {
                // Ottosson's public-domain Oklab inverse matrix, also used by GPUI's shaders.
                // https://bottosson.github.io/posts/oklab/#converting-from-linear-srgb-to-oklab
                let l = (x + 0.39633778 * y + 0.21580376 * z).powi(3);
                let m = (x - 0.105561346 * y - 0.06385417 * z).powi(3);
                let s = (x - 0.08948418 * y - 1.2914855 * z).powi(3);
                [
                    4.0767417 * l - 3.3077116 * m + 0.23096994 * s,
                    -1.268438 * l + 2.6097574 * m - 0.3413194 * s,
                    -0.0041960863 * l - 0.7034186 * m + 1.7076147 * s,
                ]
                .map(|value| {
                    if value <= 0.0031308 {
                        value * 12.92
                    } else {
                        1.055 * value.powf(1. / 2.4) - 0.055
                    }
                })
            }
        };
        gpui::Rgba {
            r: r.clamp(0., 1.),
            g: g.clamp(0., 1.),
            b: b.clamp(0., 1.),
            a: alpha.clamp(0., 1.),
        }
        .into()
    }
    fn components(color: gpui::Hsla, space: gpui::ColorSpace) -> [f32; 4] {
        let color = color.to_rgb();
        let mut channels = [color.r, color.g, color.b];
        if space == gpui::ColorSpace::Oklab {
            let [r, g, b] = channels.map(|value| {
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            });
            let l = (0.41222146 * r + 0.53633255 * g + 0.051445995 * b).cbrt();
            let m = (0.2119035 * r + 0.6806995 * g + 0.10739696 * b).cbrt();
            let s = (0.08830246 * r + 0.28171885 * g + 0.6299787 * b).cbrt();
            channels = [
                0.21045426 * l + 0.7936178 * m - 0.004072047 * s,
                1.9779985 * l - 2.4285922 * m + 0.4505937 * s,
                0.025904037 * l + 0.78277177 * m - 0.80867577 * s,
            ];
        }
        [
            channels[0] * color.a,
            channels[1] * color.a,
            channels[2] * color.a,
            color.a,
        ]
    }
}
