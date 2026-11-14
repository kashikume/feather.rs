//! Numbers of an OBJ file kept exactly as written: `mantissa / 10^decimals`.

/// A decimal number `mantissa / 10^decimals`, with no trailing zeros in the mantissa unless
/// `decimals` is 0: `0.50000` is `5 / 10^1`, `1.0000` is `1 / 10^0`, `-0.0` is `0 / 10^0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decimal {
    pub mantissa: i64,
    pub decimals: u32,
}

/// Numbers with more decimals than this are not kept as decimals (they'd never fit in an
/// `i32` once scaled anyway).
const MAX_DECIMALS: i64 = 40;

impl Decimal {
    pub const ZERO: Decimal = Decimal {
        mantissa: 0,
        decimals: 0,
    };

    /// `n / 10^decimals`, normalized.
    pub fn new(n: i64, decimals: u32) -> Decimal {
        let mut decimal = Decimal {
            mantissa: n,
            decimals,
        };
        decimal.normalize();
        decimal
    }

    fn normalize(&mut self) {
        if self.mantissa == 0 {
            self.decimals = 0;
        }
        while self.decimals > 0 && self.mantissa % 10 == 0 {
            self.mantissa /= 10;
            self.decimals -= 1;
        }
    }

    /// Parses a number as OBJ files write them: `12`, `-0.5`, `.5`, `1.`, `+3`, `1.5e-07`,
    /// `2E3`. `None` for anything else (`nan`, `inf`, ...) and for numbers that need more than
    /// 18 significant digits or [`MAX_DECIMALS`] decimals.
    pub fn parse(text: &str) -> Option<Decimal> {
        let bytes = text.as_bytes();
        let mut i = 0;
        let negative = match bytes.first() {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };

        let mut mantissa: u64 = 0;
        let mut digits = 0; // significant digits in the mantissa
                            // Zeros after the last non-zero digit, not in the mantissa (yet): only significant if
                            // another non-zero digit follows.
        let mut pending_zeros: i64 = 0;
        let mut any_digit = false;
        let mut fraction_digits: i64 = 0;
        let mut in_fraction = false;
        while i < bytes.len() {
            match bytes[i] {
                c @ b'0'..=b'9' => {
                    any_digit = true;
                    if c == b'0' {
                        if mantissa != 0 {
                            pending_zeros += 1;
                        }
                    } else {
                        digits += pending_zeros + 1;
                        if digits > 18 {
                            return None;
                        }
                        mantissa =
                            mantissa * 10u64.pow(pending_zeros as u32 + 1) + (c - b'0') as u64;
                        pending_zeros = 0;
                    }
                    if in_fraction {
                        fraction_digits += 1;
                    }
                }
                b'.' if !in_fraction => in_fraction = true,
                b'e' | b'E' => break,
                _ => return None,
            }
            i += 1;
        }
        if !any_digit {
            return None;
        }

        let mut exponent: i64 = 0;
        if i < bytes.len() {
            // At 'e' / 'E'.
            let exponent_text = &text[i + 1..];
            let exponent_digits = exponent_text
                .strip_prefix(['+', '-'])
                .unwrap_or(exponent_text);
            if exponent_digits.is_empty() || !exponent_digits.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            exponent = exponent_text.parse::<i64>().ok()?.clamp(-1000, 1000);
        }

        let mut mantissa = mantissa as i64;
        if mantissa == 0 {
            return Some(Decimal::ZERO);
        }
        // The pending zeros are trailing zeros: dropping them takes as many decimals away.
        let mut decimals = fraction_digits - exponent - pending_zeros;
        if decimals < 0 {
            mantissa = mantissa.checked_mul(10i64.checked_pow((-decimals) as u32)?)?;
            decimals = 0;
        }
        if decimals > MAX_DECIMALS {
            return None;
        }
        Some(Decimal {
            mantissa: if negative { -mantissa } else { mantissa },
            decimals: decimals as u32,
        })
    }

    /// The integer `n` with `n / 10^decimals` equal to this number, if it has at most
    /// `decimals` decimals and `n` fits in an `i64`.
    pub fn scaled(self, decimals: u32) -> Option<i64> {
        let shift = decimals.checked_sub(self.decimals)?;
        self.mantissa.checked_mul(10i64.checked_pow(shift)?)
    }

    /// `1 - self`, exactly (`None` if it doesn't fit).
    pub fn one_minus(self) -> Option<Decimal> {
        let one = 10i64.checked_pow(self.decimals)?;
        Some(Decimal::new(one.checked_sub(self.mantissa)?, self.decimals))
    }

    /// The nearest `f32`, as parsing the number's text gives it.
    pub fn to_f32(self) -> f32 {
        format!("{}e-{}", self.mantissa, self.decimals)
            .parse()
            .unwrap()
    }

    /// The nearest `f64`.
    pub fn to_f64(self) -> f64 {
        format!("{}e-{}", self.mantissa, self.decimals)
            .parse()
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(mantissa: i64, decimals: u32) -> Option<Decimal> {
        Some(Decimal { mantissa, decimals })
    }

    #[test]
    fn parses_obj_numbers() {
        assert_eq!(Decimal::parse("0.573651"), d(573651, 6));
        assert_eq!(Decimal::parse("-111.966"), d(-111966, 3));
        assert_eq!(Decimal::parse("12"), d(12, 0));
        assert_eq!(Decimal::parse("+3"), d(3, 0));
        assert_eq!(Decimal::parse(".5"), d(5, 1));
        assert_eq!(Decimal::parse("1."), d(1, 0));
        assert_eq!(Decimal::parse("-.25"), d(-25, 2));
    }

    #[test]
    fn drops_trailing_zeros() {
        assert_eq!(Decimal::parse("0.50000"), d(5, 1));
        assert_eq!(Decimal::parse("1.0000"), d(1, 0));
        assert_eq!(Decimal::parse("100"), d(100, 0));
        assert_eq!(Decimal::parse("100.00"), d(100, 0));
        assert_eq!(Decimal::parse("0.00000"), d(0, 0));
        assert_eq!(Decimal::parse("-0.0000"), d(0, 0));
        assert_eq!(Decimal::parse("000.0100"), d(1, 2));
        // Trailing zeros don't count against the 18 significant digits.
        assert_eq!(Decimal::parse("1.00000000000000000000000"), d(1, 0));
        assert_eq!(Decimal::parse("1.000000000000000000000001"), None);
    }

    #[test]
    fn parses_exponents() {
        assert_eq!(Decimal::parse("1.5e-07"), d(15, 8));
        assert_eq!(Decimal::parse("1.5E-7"), d(15, 8));
        assert_eq!(Decimal::parse("2e3"), d(2000, 0));
        assert_eq!(Decimal::parse("2.5e+1"), d(25, 0));
        assert_eq!(Decimal::parse("-1.25e1"), d(-125, 1));
        assert_eq!(Decimal::parse("0e-5"), d(0, 0));
    }

    #[test]
    fn rejects_what_isnt_a_plain_number() {
        for text in [
            "", "-", ".", "e5", "1e", "1e+", "nan", "inf", "-inf", "1.2.3", "1,5", "0x10", "1e5.0",
        ] {
            assert_eq!(Decimal::parse(text), None, "{text}");
        }
        // Too many significant digits or decimals.
        assert_eq!(Decimal::parse("1.2345678901234567891"), None);
        assert_eq!(Decimal::parse("1e-50"), None);
        assert_eq!(Decimal::parse("1e30"), None);
        // Leading zeros don't count as significant.
        assert_eq!(Decimal::parse("0.000000000000000001"), d(1, 18));
    }

    #[test]
    fn scales() {
        let x = Decimal::parse("-0.25").unwrap();
        assert_eq!(x.scaled(2), Some(-25));
        assert_eq!(x.scaled(5), Some(-25000));
        assert_eq!(x.scaled(1), None);
        assert_eq!(Decimal::parse("9").unwrap().scaled(19), None);
    }

    #[test]
    fn one_minus_is_exact() {
        assert_eq!(
            Decimal::parse("0.637534").unwrap().one_minus(),
            d(362466, 6)
        );
        assert_eq!(Decimal::parse("0.25").unwrap().one_minus(), d(75, 2));
        assert_eq!(Decimal::ZERO.one_minus(), d(1, 0));
        assert_eq!(Decimal::new(1, 0).one_minus(), Some(Decimal::ZERO));
        assert_eq!(Decimal::parse("1.5").unwrap().one_minus(), d(-5, 1));
    }

    #[test]
    fn converts_to_floats_like_parsing() {
        for text in ["0.573651", "-111.966", "0.1", "1.5e-07", "3"] {
            let decimal = Decimal::parse(text).unwrap();
            assert_eq!(decimal.to_f32(), text.parse::<f32>().unwrap());
            assert_eq!(decimal.to_f64(), text.parse::<f64>().unwrap());
        }
    }
}
