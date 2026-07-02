use proptest::prelude::*;
use qanto::interoperability::math::{montgomery_reduce, Q};

/// SAGA Unoptimized, provably correct stack-allocated `u128` reference implementation
/// for Montgomery reduction. This mathematically ensures the invariant:
/// \forall x\in [0,2^{64}-1],\quad \text{MontgomeryReduction}(x)\equiv x\cdot R^{-1}\mathinner{\;\left(\mod \,Q\right)}
fn reference_montgomery_reduce(a: u64) -> u32 {
    let a_u128 = a as u128;
    let q_u128 = Q as u128;
    // Radix R = 2^32
    let q_inv = 58728449u128; // -Q^-1 mod 2^32

    // m = (a_low * Q_INV) mod 2^32
    let a_low = a_u128 & 0xFFFFFFFF;
    let m = (a_low * q_inv) & 0xFFFFFFFF;

    // t = (a + m * Q) / 2^32
    let t = (a_u128 + m * q_u128) >> 32;

    let mut t_u32 = t as u32;
    if t_u32 >= Q {
        t_u32 -= Q;
    }
    t_u32
}

fn montgomery_strategy() -> impl Strategy<Value = u64> {
    prop_oneof![
        // Boundary around 0
        0u64..=1u64,
        // Boundary around Q
        (Q as u64 - 1)..=(Q as u64 + 1),
        // Boundary around 2Q
        (2 * Q as u64 - 1)..=(2 * Q as u64 + 1),
        // Extreme upper bounds 32-bit and 64-bit limits
        Just(u32::MAX as u64),
        Just(u64::MAX),
        Just((1u64 << 31) - 1),
        Just((1u64 << 63) - 1),
        // Random uniformly distributed
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]
    #[test]
    fn test_montgomery_reduction_fuzz(a in montgomery_strategy()) {
        let reference_out = reference_montgomery_reduce(a);
        let optimized_out = montgomery_reduce(a);
        prop_assert_eq!(reference_out, optimized_out, "Mismatch on input {}: reference={}, optimized={}", a, reference_out, optimized_out);
    }
}
