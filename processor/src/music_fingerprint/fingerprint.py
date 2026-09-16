"""Landmark generation: pair anchor peaks with nearby target peaks inside a
target zone and hash each pair into a compact fingerprint."""

from __future__ import annotations

from dataclasses import dataclass

from music_fingerprint.audio import AudioBuffer
from music_fingerprint.config import FINGERPRINT_ALGORITHM_VERSION, FingerprintConfig
from music_fingerprint.constellation import ConstellationMap, build_constellation
from music_fingerprint.hashing import pack_hash


@dataclass(frozen=True, slots=True)
class Fingerprint:
    hash: int
    offset_ms: int  # anchor peak's absolute time offset into the track


def _generate_landmarks(
    constellation: ConstellationMap, config: FingerprintConfig
) -> list[Fingerprint]:
    peaks = constellation.peaks
    n = len(peaks)
    fingerprints: list[Fingerprint] = []

    for i in range(n):
        anchor = peaks[i]
        found = 0
        j = i + 1
        while j < n and found < config.fanout:
            target = peaks[j]
            dt = target.time_bin - anchor.time_bin
            if dt > config.target_zone_max_frames:
                break  # peaks are time-sorted: no later j can be in range either
            if dt >= config.target_zone_min_frames:
                h = pack_hash(
                    anchor.frequency_bin,
                    target.frequency_bin,
                    dt,
                    config,
                    version=FINGERPRINT_ALGORITHM_VERSION,
                )
                fingerprints.append(
                    Fingerprint(hash=h, offset_ms=constellation.time_bin_to_ms(anchor.time_bin))
                )
                found += 1
            j += 1

    return fingerprints


def generate_fingerprints(
    audio: AudioBuffer, config: FingerprintConfig | None = None
) -> list[Fingerprint]:
    """Full pipeline: canonical audio -> constellation -> landmark hashes.

    Deterministic: identical input audio and config always produce an
    identical fingerprint list (same order too), which golden fixture tests
    rely on.
    """
    cfg = config or FingerprintConfig()
    constellation = build_constellation(audio, cfg)
    return _generate_landmarks(constellation, cfg)
