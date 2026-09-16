"""Local-maximum spectral peak detection over a magnitude spectrogram."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy.ndimage import maximum_filter, uniform_filter

from music_fingerprint.config import FingerprintConfig
from music_fingerprint.spectrogram import Spectrogram


@dataclass(frozen=True, slots=True)
class Peak:
    time_bin: int  # STFT frame index
    frequency_bin: int  # absolute FFT bin index (already offset-corrected)
    amplitude_db: float


def detect_peaks(
    spec: Spectrogram, config: FingerprintConfig, duration_seconds: float
) -> list[Peak]:
    """Detect strong, spatially-isolated local maxima ("constellation" points).

    A bin is kept as a peak iff:
      1. it is the maximum within its (freq, time) neighborhood window
         (enforces spatial isolation / prevents redundant clustered peaks),
      2. it exceeds the neighborhood's local mean by at least
         `peak_min_db_above_local_mean` dB (relative contrast — adapts to
         locally loud/quiet passages instead of one global threshold), and
      3. it exceeds `peak_absolute_floor_db` relative to the track's own
         peak level (absolute floor — keeps near-silence from producing
         peaks out of numerical noise).

    Finally, density is capped at `max_peaks_per_second` (ranked by
    amplitude) to bound fingerprint generation against pathological inputs
    (e.g. white noise or clipping, which are locally "peaky" everywhere).
    """
    magnitude_db = spec.magnitude_db
    if magnitude_db.size == 0:
        return []

    neighborhood = (config.peak_neighborhood_freq, config.peak_neighborhood_time)

    local_max = maximum_filter(magnitude_db, size=neighborhood, mode="constant", cval=-np.inf)
    is_local_max = magnitude_db >= local_max

    # A local *mean* (uniform_filter, a separable box filter) rather than a
    # true local median: ~400x faster in practice (measured: 6.1s vs 15ms
    # on a ~10s-clip-sized spectrogram with this neighborhood size — see
    # docs/performance.md) because scipy's median_filter has no equivalent
    # separable fast path. The mean is a slightly less outlier-robust
    # background estimate than the median, but empirically produces the
    # same peak selection behavior for our purposes (verified by re-running
    # the full noise/compression robustness test suite after this change).
    local_mean = uniform_filter(magnitude_db, size=neighborhood, mode="reflect")
    has_contrast = magnitude_db >= (local_mean + config.peak_min_db_above_local_mean)

    global_max = float(np.max(magnitude_db))
    above_floor = magnitude_db >= (global_max + config.peak_absolute_floor_db)

    mask = is_local_max & has_contrast & above_floor
    freq_idx, time_idx = np.nonzero(mask)

    peaks = [
        Peak(
            time_bin=int(t),
            frequency_bin=int(f) + spec.bin_offset,
            amplitude_db=float(magnitude_db[f, t]),
        )
        for f, t in zip(freq_idx, time_idx, strict=True)
    ]

    max_peaks = max(1, int(config.max_peaks_per_second * max(duration_seconds, 0.01)))
    if len(peaks) > max_peaks:
        peaks.sort(key=lambda p: p.amplitude_db, reverse=True)
        peaks = peaks[:max_peaks]

    peaks.sort(key=lambda p: (p.time_bin, p.frequency_bin))
    return peaks
