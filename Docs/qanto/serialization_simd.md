# Zero-Copy Serialization with SIMD

Qanto’s custom `qanto_serde` uses a compact binary format and zero-copy deserialization. On supported platforms, enable SIMD to accelerate core operations.

## Enable Target Features

- x86_64 (AVX2): `RUSTFLAGS="-C target-feature=+avx2"`
- ARM64 (NEON): `RUSTFLAGS="-C target-feature=+neon"`

Set per build:

```
RUSTFLAGS="-C target-feature=+avx2" cargo build --release
```

## Guidelines

- Keep buffers aligned to 32 bytes for AVX2-friendly loads.
- Prefer `copy_nonoverlapping` for bulk buffer moves.
- Validate endianness and alignment assumptions in tests for determinism.
- Use feature gates to keep portable fallbacks for non-SIMD targets.

## Notes

- Avoid heavy external SIMD crates; use `std::arch` intrinsics if needed.
- Stick to deterministic byte layouts for consensus-critical structures.