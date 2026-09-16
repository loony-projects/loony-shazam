"""Constellation map: the sparse set of (time, frequency) peaks used as the
basis for landmark hashing. Deterministic — no learned components."""

from __future__ import annotations

from dataclasses import dataclass

from music_fingerprint.audio import AudioBuffer
from music_fingerprint.config import FingerprintConfig
from music_fingerprint.peaks import Peak, detect_peaks
from music_fingerprint.spectrogram import compute_spectrogram


@dataclass(frozen=True, slots=True)
class ConstellationMap:
    peaks: list[Peak]  # sorted by (time_bin, frequency_bin)
    hop_size: int
    sample_rate: int

    def time_bin_to_ms(self, time_bin: int) -> int:
        return round(time_bin * self.hop_size * 1000 / self.sample_rate)


def build_constellation(audio: AudioBuffer, config: FingerprintConfig) -> ConstellationMap:
    spec = compute_spectrogram(audio, config)
    peaks = detect_peaks(spec, config, audio.duration_seconds)
    return ConstellationMap(peaks=peaks, hop_size=spec.hop_size, sample_rate=spec.sample_rate)
