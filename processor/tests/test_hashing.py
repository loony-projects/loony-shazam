from __future__ import annotations

from music_fingerprint.config import FingerprintConfig
from music_fingerprint.hashing import pack_hash, quantize_frequency_bin, unpack_hash


def test_pack_unpack_roundtrip():
    config = FingerprintConfig()
    h = pack_hash(
        anchor_freq_bin=100, target_freq_bin=300, delta_t_frames=42, config=config, version=1
    )
    unpacked = unpack_hash(h, config)

    assert unpacked.version == 1
    assert unpacked.anchor_freq_q == quantize_frequency_bin(100, config)
    assert unpacked.target_freq_q == quantize_frequency_bin(300, config)
    assert unpacked.delta_t == 42


def test_hash_fits_in_32_bits():
    config = FingerprintConfig()
    h = pack_hash(config.max_freq_bin, config.max_freq_bin, 255, config, version=15)
    assert 0 <= h < (1 << 32)


def test_deterministic_same_inputs_same_hash():
    config = FingerprintConfig()
    h1 = pack_hash(50, 200, 10, config)
    h2 = pack_hash(50, 200, 10, config)
    assert h1 == h2


def test_different_inputs_generally_different_hashes():
    config = FingerprintConfig()
    h1 = pack_hash(50, 200, 10, config)
    h2 = pack_hash(51, 200, 10, config)
    h3 = pack_hash(50, 201, 10, config)
    h4 = pack_hash(50, 200, 11, config)
    assert len({h1, h2, h3, h4}) >= 2  # quantization may merge some adjacent bins


def test_delta_t_clamped_to_bit_width():
    config = FingerprintConfig()
    h = pack_hash(10, 20, delta_t_frames=99999, config=config)
    unpacked = unpack_hash(h, config)
    assert unpacked.delta_t == (1 << config.delta_bits) - 1


def test_version_out_of_range_rejected():
    import pytest

    config = FingerprintConfig()
    with pytest.raises(ValueError):
        pack_hash(10, 20, 5, config, version=16)


def test_quantize_frequency_bin_is_monotonic_and_bounded():
    config = FingerprintConfig()
    prev = -1
    for b in range(config.min_freq_bin, config.max_freq_bin + 1, 7):
        q = quantize_frequency_bin(b, config)
        assert 0 <= q < (1 << config.freq_bits)
        assert q >= prev
        prev = q
