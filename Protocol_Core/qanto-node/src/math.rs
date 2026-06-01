use crate::{QANTO_SCALE, SCORE_SCALE};

/// Deterministic multiplication for fixed-point math with SCORE_SCALE (i128).
/// Formula: (a * b) / SCORE_SCALE
pub fn mul_scale(a: i128, b: i128) -> i128 {
    a.checked_mul(b)
        .map(|res| res / SCORE_SCALE)
        .unwrap_or(0)
}

/// Deterministic multiplication for fixed-point math with QANTO_SCALE (u128).
/// Formula: (a * b) / QANTO_SCALE
pub fn mul_scale_u128(a: u128, b: u128) -> u128 {
    a.checked_mul(b)
        .map(|res| res / QANTO_SCALE)
        .unwrap_or(0)
}

/// Deterministic division for fixed-point math with SCORE_SCALE (i128).
/// Formula: (a * SCORE_SCALE) / b
pub fn div_scale(a: i128, b: i128) -> i128 {
    if b == 0 {
        return 0;
    }
    a.checked_mul(SCORE_SCALE)
        .map(|res| res / b)
        .unwrap_or(0)
}

/// Deterministic division for fixed-point math with QANTO_SCALE (u128).
/// Formula: (a * QANTO_SCALE) / b
pub fn div_scale_u128(a: u128, b: u128) -> u128 {
    if b == 0 {
        return 0;
    }
    a.checked_mul(QANTO_SCALE)
        .map(|res| res / b)
        .unwrap_or(0)
}

/// Saturating multiplication for fixed-point math to prevent overflow.
pub fn saturating_mul_scale(a: i128, b: i128) -> i128 {
    a.saturating_mul(b) / SCORE_SCALE
}

/// Deterministic integer square root for u128.
/// Useful for distance and standard deviation calculations.
pub fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
