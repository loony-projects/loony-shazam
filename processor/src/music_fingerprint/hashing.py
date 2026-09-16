"""Deterministic, compact landmark hash packing.

Layout of the 32-bit hash (see docs/fingerprinting.md for full rationale):

    bit:   31        28 27        18 17         8 7           0
          [ version:4 ][ anchor_f:10 ][ target_f:10 ][ delta_t:8 ]

- `version` is `FINGERPRINT_ALGORITHM_VERSION`, so hashes from incompatible
  algorithm revisions can never collide even if ever compared directly.
- `anchor_f`/`target_f` are the anchor/target peak's FFT bin, quantized
  into the configured bit width across the analysis band
  [min_frequency_hz, max_frequency_hz].
- `delta_t` is the frame distance between anchor and target, clamped to the
  configured bit width (which must cover `target_zone_max_frames`).

The packed value fits in an unsigned 32-bit integer and is stored as a
Postgres BIGINT (there is room to grow the layout to 64 bits later without
a schema change).
"""

from __future__ import annotations

from dataclasses import dataclass

from music_fingerprint.config import FINGERPRINT_ALGORITHM_VERSION, FingerprintConfig


@dataclass(frozen=True, slots=True)
class UnpackedHash:
    version: int
    anchor_freq_q: int
    target_freq_q: int
    delta_t: int


def _freq_max_q(config: FingerprintConfig) -> int:
    return (1 << config.freq_bits) - 1


def _delta_max(config: FingerprintConfig) -> int:
    return (1 << config.delta_bits) - 1


def quantize_frequency_bin(bin_index: int, config: FingerprintConfig) -> int:
    """Map an absolute FFT bin index (within the analysis band) to a
    `config.freq_bits`-wide quantized value. Deterministic integer math —
    no floating point rounding drift between runs."""
    low, high = config.min_freq_bin, config.max_freq_bin
    span = max(1, high - low)
    max_q = _freq_max_q(config)
    offset = min(max(bin_index - low, 0), span)
    return (offset * max_q) // span


def pack_hash(
    anchor_freq_bin: int,
    target_freq_bin: int,
    delta_t_frames: int,
    config: FingerprintConfig,
    version: int = FINGERPRINT_ALGORITHM_VERSION,
) -> int:
    if not (0 <= version < (1 << config.version_bits)):
        raise ValueError(f"algorithm version {version} does not fit in {config.version_bits} bits")

    f1 = quantize_frequency_bin(anchor_freq_bin, config)
    f2 = quantize_frequency_bin(target_freq_bin, config)
    dt = min(max(delta_t_frames, 0), _delta_max(config))

    shift_f2 = config.delta_bits
    shift_f1 = config.delta_bits + config.freq_bits
    shift_version = config.delta_bits + config.freq_bits * 2

    return (version << shift_version) | (f1 << shift_f1) | (f2 << shift_f2) | dt


def unpack_hash(hash_value: int, config: FingerprintConfig) -> UnpackedHash:
    """Inverse of `pack_hash`, for tests/debugging (not on the hot path)."""
    delta_mask = _delta_max(config)
    freq_mask = _freq_max_q(config)

    shift_f2 = config.delta_bits
    shift_f1 = config.delta_bits + config.freq_bits
    shift_version = config.delta_bits + config.freq_bits * 2

    return UnpackedHash(
        version=(hash_value >> shift_version) & ((1 << config.version_bits) - 1),
        anchor_freq_q=(hash_value >> shift_f1) & freq_mask,
        target_freq_q=(hash_value >> shift_f2) & freq_mask,
        delta_t=hash_value & delta_mask,
    )
