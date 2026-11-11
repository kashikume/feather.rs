//! Easing functions: map an animation's progress `p` (0 at the start, 1 at the end) to the share
//! of the change to apply, so motion can speed up, slow down, overshoot or bounce instead of
//! moving at a constant speed.
//!
//! Every function gives 0 at 0 and 1 at 1 (up to rounding). In between, the `*_ease_in`
//! variants start slowly, the `*_ease_out` variants end slowly (each is the mirror image of its
//! `ease_in`: `out(p) == 1 - in(1 - p)`), and the `*_ease_in_out` variants do both, passing 0.5
//! at 0.5. Elastic and back ones go below 0 or above 1 on the way; bounce ones stay within 0..1
//! but go back and forth.
//!
//! `Easing` names each function, for `SlideAnimation::easing`.
//!
//! The formulas are those of the C++ Feather engine (`feather-Utils.h`). Its `easeInOut3` is the
//! same function as `cubic_ease_in_out`.

use std::f32::consts::{FRAC_PI_2, PI};

pub fn linear(p: f32) -> f32 {
    p
}

pub fn quadratic_ease_in(p: f32) -> f32 {
    p * p
}

pub fn quadratic_ease_out(p: f32) -> f32 {
    -(p * (p - 2.0))
}

pub fn quadratic_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        2.0 * p * p
    } else {
        (-2.0 * p * p) + (4.0 * p) - 1.0
    }
}

pub fn cubic_ease_in(p: f32) -> f32 {
    p * p * p
}

pub fn cubic_ease_out(p: f32) -> f32 {
    let f = p - 1.0;
    f * f * f + 1.0
}

pub fn cubic_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        4.0 * p * p * p
    } else {
        let f = (2.0 * p) - 2.0;
        0.5 * f * f * f + 1.0
    }
}

pub fn quartic_ease_in(p: f32) -> f32 {
    p * p * p * p
}

pub fn quartic_ease_out(p: f32) -> f32 {
    let f = p - 1.0;
    f * f * f * (1.0 - p) + 1.0
}

pub fn quartic_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        8.0 * p * p * p * p
    } else {
        let f = p - 1.0;
        -8.0 * f * f * f * f + 1.0
    }
}

pub fn quintic_ease_in(p: f32) -> f32 {
    p * p * p * p * p
}

pub fn quintic_ease_out(p: f32) -> f32 {
    let f = p - 1.0;
    f * f * f * f * f + 1.0
}

pub fn quintic_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        16.0 * p * p * p * p * p
    } else {
        let f = (2.0 * p) - 2.0;
        0.5 * f * f * f * f * f + 1.0
    }
}

pub fn sine_ease_in(p: f32) -> f32 {
    ((p - 1.0) * FRAC_PI_2).sin() + 1.0
}

pub fn sine_ease_out(p: f32) -> f32 {
    (p * FRAC_PI_2).sin()
}

pub fn sine_ease_in_out(p: f32) -> f32 {
    0.5 * (1.0 - (p * PI).cos())
}

pub fn circular_ease_in(p: f32) -> f32 {
    1.0 - (1.0 - (p * p)).sqrt()
}

pub fn circular_ease_out(p: f32) -> f32 {
    ((2.0 - p) * p).sqrt()
}

pub fn circular_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        0.5 * (1.0 - (1.0 - 4.0 * (p * p)).sqrt())
    } else {
        0.5 * ((-((2.0 * p) - 3.0) * ((2.0 * p) - 1.0)).sqrt() + 1.0)
    }
}

pub fn exponential_ease_in(p: f32) -> f32 {
    if p == 0.0 {
        p
    } else {
        2.0f32.powf(10.0 * (p - 1.0))
    }
}

pub fn exponential_ease_out(p: f32) -> f32 {
    if p == 1.0 {
        p
    } else {
        1.0 - 2.0f32.powf(-10.0 * p)
    }
}

pub fn exponential_ease_in_out(p: f32) -> f32 {
    if p == 0.0 || p == 1.0 {
        return p;
    }
    if p < 0.5 {
        0.5 * 2.0f32.powf((20.0 * p) - 10.0)
    } else {
        -0.5 * 2.0f32.powf((-20.0 * p) + 10.0) + 1.0
    }
}

pub fn elastic_ease_in(p: f32) -> f32 {
    (13.0 * FRAC_PI_2 * p).sin() * 2.0f32.powf(10.0 * (p - 1.0))
}

pub fn elastic_ease_out(p: f32) -> f32 {
    (-13.0 * FRAC_PI_2 * (p + 1.0)).sin() * 2.0f32.powf(-10.0 * p) + 1.0
}

pub fn elastic_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        0.5 * (13.0 * FRAC_PI_2 * (2.0 * p)).sin() * 2.0f32.powf(10.0 * ((2.0 * p) - 1.0))
    } else {
        0.5 * ((-13.0 * FRAC_PI_2 * ((2.0 * p - 1.0) + 1.0)).sin()
            * 2.0f32.powf(-10.0 * (2.0 * p - 1.0))
            + 2.0)
    }
}

pub fn back_ease_in(p: f32) -> f32 {
    p * p * p - p * (p * PI).sin()
}

pub fn back_ease_out(p: f32) -> f32 {
    let f = 1.0 - p;
    1.0 - (f * f * f - f * (f * PI).sin())
}

pub fn back_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        let f = 2.0 * p;
        0.5 * (f * f * f - f * (f * PI).sin())
    } else {
        let f = 1.0 - (2.0 * p - 1.0);
        0.5 * (1.0 - (f * f * f - f * (f * PI).sin())) + 0.5
    }
}

pub fn bounce_ease_out(p: f32) -> f32 {
    if p < 4.0 / 11.0 {
        (121.0 * p * p) / 16.0
    } else if p < 8.0 / 11.0 {
        (363.0 / 40.0 * p * p) - (99.0 / 10.0 * p) + 17.0 / 5.0
    } else if p < 9.0 / 10.0 {
        (4356.0 / 361.0 * p * p) - (35442.0 / 1805.0 * p) + 16061.0 / 1805.0
    } else {
        (54.0 / 5.0 * p * p) - (513.0 / 25.0 * p) + 268.0 / 25.0
    }
}

pub fn bounce_ease_in(p: f32) -> f32 {
    1.0 - bounce_ease_out(1.0 - p)
}

pub fn bounce_ease_in_out(p: f32) -> f32 {
    if p < 0.5 {
        0.5 * bounce_ease_in(p * 2.0)
    } else {
        0.5 * bounce_ease_out(p * 2.0 - 1.0) + 0.5
    }
}

/// One of the easing functions of this module.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Easing {
    #[default]
    Linear,
    QuadraticIn,
    QuadraticOut,
    QuadraticInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    QuarticIn,
    QuarticOut,
    QuarticInOut,
    QuinticIn,
    QuinticOut,
    QuinticInOut,
    SineIn,
    SineOut,
    SineInOut,
    CircularIn,
    CircularOut,
    CircularInOut,
    ExponentialIn,
    ExponentialOut,
    ExponentialInOut,
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    BackIn,
    BackOut,
    BackInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
}

impl Easing {
    /// Every easing, `Linear` first, then each kind's in, out and in-out variants.
    pub const ALL: [Easing; 31] = [
        Easing::Linear,
        Easing::QuadraticIn,
        Easing::QuadraticOut,
        Easing::QuadraticInOut,
        Easing::CubicIn,
        Easing::CubicOut,
        Easing::CubicInOut,
        Easing::QuarticIn,
        Easing::QuarticOut,
        Easing::QuarticInOut,
        Easing::QuinticIn,
        Easing::QuinticOut,
        Easing::QuinticInOut,
        Easing::SineIn,
        Easing::SineOut,
        Easing::SineInOut,
        Easing::CircularIn,
        Easing::CircularOut,
        Easing::CircularInOut,
        Easing::ExponentialIn,
        Easing::ExponentialOut,
        Easing::ExponentialInOut,
        Easing::ElasticIn,
        Easing::ElasticOut,
        Easing::ElasticInOut,
        Easing::BackIn,
        Easing::BackOut,
        Easing::BackInOut,
        Easing::BounceIn,
        Easing::BounceOut,
        Easing::BounceInOut,
    ];

    /// The function this easing names.
    pub fn function(self) -> fn(f32) -> f32 {
        match self {
            Easing::Linear => linear,
            Easing::QuadraticIn => quadratic_ease_in,
            Easing::QuadraticOut => quadratic_ease_out,
            Easing::QuadraticInOut => quadratic_ease_in_out,
            Easing::CubicIn => cubic_ease_in,
            Easing::CubicOut => cubic_ease_out,
            Easing::CubicInOut => cubic_ease_in_out,
            Easing::QuarticIn => quartic_ease_in,
            Easing::QuarticOut => quartic_ease_out,
            Easing::QuarticInOut => quartic_ease_in_out,
            Easing::QuinticIn => quintic_ease_in,
            Easing::QuinticOut => quintic_ease_out,
            Easing::QuinticInOut => quintic_ease_in_out,
            Easing::SineIn => sine_ease_in,
            Easing::SineOut => sine_ease_out,
            Easing::SineInOut => sine_ease_in_out,
            Easing::CircularIn => circular_ease_in,
            Easing::CircularOut => circular_ease_out,
            Easing::CircularInOut => circular_ease_in_out,
            Easing::ExponentialIn => exponential_ease_in,
            Easing::ExponentialOut => exponential_ease_out,
            Easing::ExponentialInOut => exponential_ease_in_out,
            Easing::ElasticIn => elastic_ease_in,
            Easing::ElasticOut => elastic_ease_out,
            Easing::ElasticInOut => elastic_ease_in_out,
            Easing::BackIn => back_ease_in,
            Easing::BackOut => back_ease_out,
            Easing::BackInOut => back_ease_in_out,
            Easing::BounceIn => bounce_ease_in,
            Easing::BounceOut => bounce_ease_out,
            Easing::BounceInOut => bounce_ease_in_out,
        }
    }

    /// Applies the easing to progress `p`. Gives exactly 0 for `p <= 0` and exactly 1 for
    /// `p >= 1`, so an animation starts and ends exactly where it should, whatever the
    /// function's rounding.
    pub fn apply(self, p: f32) -> f32 {
        if p <= 0.0 {
            0.0
        } else if p >= 1.0 {
            1.0
        } else {
            self.function()(p)
        }
    }

    /// Readable name, e.g. "Cubic in-out".
    pub fn name(self) -> &'static str {
        match self {
            Easing::Linear => "Linear",
            Easing::QuadraticIn => "Quadratic in",
            Easing::QuadraticOut => "Quadratic out",
            Easing::QuadraticInOut => "Quadratic in-out",
            Easing::CubicIn => "Cubic in",
            Easing::CubicOut => "Cubic out",
            Easing::CubicInOut => "Cubic in-out",
            Easing::QuarticIn => "Quartic in",
            Easing::QuarticOut => "Quartic out",
            Easing::QuarticInOut => "Quartic in-out",
            Easing::QuinticIn => "Quintic in",
            Easing::QuinticOut => "Quintic out",
            Easing::QuinticInOut => "Quintic in-out",
            Easing::SineIn => "Sine in",
            Easing::SineOut => "Sine out",
            Easing::SineInOut => "Sine in-out",
            Easing::CircularIn => "Circular in",
            Easing::CircularOut => "Circular out",
            Easing::CircularInOut => "Circular in-out",
            Easing::ExponentialIn => "Exponential in",
            Easing::ExponentialOut => "Exponential out",
            Easing::ExponentialInOut => "Exponential in-out",
            Easing::ElasticIn => "Elastic in",
            Easing::ElasticOut => "Elastic out",
            Easing::ElasticInOut => "Elastic in-out",
            Easing::BackIn => "Back in",
            Easing::BackOut => "Back out",
            Easing::BackInOut => "Back in-out",
            Easing::BounceIn => "Bounce in",
            Easing::BounceOut => "Bounce out",
            Easing::BounceInOut => "Bounce in-out",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    /// Progress values from 0 to 1 in small steps.
    fn steps() -> impl Iterator<Item = f32> {
        (0..=1000).map(|i| i as f32 / 1000.0)
    }

    /// (in, out, in-out) triples; `Linear` is its own mirror.
    fn kinds() -> Vec<(Easing, Easing, Easing)> {
        Easing::ALL[1..]
            .chunks(3)
            .map(|c| (c[0], c[1], c[2]))
            .collect()
    }

    #[test]
    fn all_lists_every_easing_once() {
        let mut seen = std::collections::HashSet::new();
        for e in Easing::ALL {
            assert!(seen.insert(e), "{e:?} listed twice");
        }
        assert_eq!(kinds().len(), 10);
        for (i, o, io) in kinds() {
            assert!(i.name().ends_with(" in"), "{i:?}");
            assert!(o.name().ends_with(" out"), "{o:?}");
            assert!(io.name().ends_with(" in-out"), "{io:?}");
        }
    }

    #[test]
    fn functions_go_from_0_to_1() {
        for e in Easing::ALL {
            let f = e.function();
            assert!(f(0.0).abs() < EPS, "{e:?}(0) = {}", f(0.0));
            assert!((f(1.0) - 1.0).abs() < EPS, "{e:?}(1) = {}", f(1.0));
        }
    }

    #[test]
    fn apply_is_exact_at_the_ends_and_clamps() {
        for e in Easing::ALL {
            assert_eq!(e.apply(0.0), 0.0, "{e:?}");
            assert_eq!(e.apply(1.0), 1.0, "{e:?}");
            assert_eq!(e.apply(-0.5), 0.0, "{e:?}");
            assert_eq!(e.apply(1.5), 1.0, "{e:?}");
            assert_eq!(e.apply(0.3), e.function()(0.3), "{e:?}");
        }
    }

    #[test]
    fn in_out_variants_pass_the_middle() {
        for (_, _, io) in kinds() {
            assert!((io.function()(0.5) - 0.5).abs() < EPS, "{io:?}");
        }
    }

    #[test]
    fn out_variants_mirror_in_variants() {
        for (i, o, _) in kinds() {
            for p in steps() {
                let mirrored = 1.0 - i.function()(1.0 - p);
                assert!(
                    (o.function()(p) - mirrored).abs() < EPS,
                    "{o:?}({p}) = {}, mirror of {i:?} = {mirrored}",
                    o.function()(p)
                );
            }
        }
    }

    #[test]
    fn in_out_variants_are_symmetric() {
        for (_, _, io) in kinds() {
            for p in steps() {
                let mirrored = 1.0 - io.function()(1.0 - p);
                assert!((io.function()(p) - mirrored).abs() < EPS, "{io:?}({p})");
            }
        }
    }

    #[test]
    fn smooth_easings_stay_in_range_and_never_decrease() {
        let overshooting = |e: Easing| e.name().starts_with("Elastic") || e.name().starts_with("Back");
        let bouncing = |e: Easing| e.name().starts_with("Bounce");
        for e in Easing::ALL.into_iter().filter(|&e| !overshooting(e)) {
            let mut previous = 0.0;
            for p in steps() {
                let v = e.function()(p);
                assert!((-EPS..=1.0 + EPS).contains(&v), "{e:?}({p}) = {v}");
                if !bouncing(e) {
                    assert!(v >= previous - EPS, "{e:?} decreases at {p}");
                }
                previous = v;
            }
        }
    }

    #[test]
    fn overshooting_easings_overshoot() {
        let min_max = |e: Easing| {
            steps()
                .map(|p| e.function()(p))
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        };
        assert!(min_max(Easing::BackIn).0 < -0.05);
        assert!(min_max(Easing::BackOut).1 > 1.05);
        assert!(min_max(Easing::ElasticIn).0 < -0.05);
        assert!(min_max(Easing::ElasticOut).1 > 1.05);
    }

    #[test]
    fn some_known_values() {
        assert_eq!(quadratic_ease_in(0.5), 0.25);
        assert_eq!(cubic_ease_out(0.5), 0.875);
        assert!((sine_ease_out(0.5) - std::f32::consts::FRAC_1_SQRT_2).abs() < EPS);
        // Second arc: 363/40 p² - 99/10 p + 17/5.
        assert!((bounce_ease_out(0.5) - 0.71875).abs() < EPS);
    }
}
