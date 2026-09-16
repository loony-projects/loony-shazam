"""STFT magnitude spectrogram computation, band-limited and dB-scaled."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy.signal import stft

from music_fingerprint.audio import AudioBuffer
from music_fingerprint.config import FingerprintConfig

#: Numerical floor to avoid log(0). Chosen far below any real signal level
#: relative to a peak-normalized [-1, 1] input.
_MAGNITUDE_EPSILON = 1e-10


@dataclass(frozen=True, slots=True)
class Spectrogram:
    #: magnitude in dB, shape (num_band_bins, num_frames), band-limited to
    #: [config.min_frequency_hz, config.max_frequency_hz]
    magnitude_db: np.ndarray
    #: absolute FFT bin index (into the full rfft output) for row i of
    #: magnitude_db — needed to map peaks back to real frequency/hash bins.
    bin_offset: int
    hop_size: int
    sample_rate: int

    @property
    def num_frames(self) -> int:
        return self.magnitude_db.shape[1]

    @property
    def num_bins(self) -> int:
        return self.magnitude_db.shape[0]


def compute_spectrogram(audio: AudioBuffer, config: FingerprintConfig) -> Spectrogram:
    """Compute a band-limited, dB-scaled magnitude spectrogram.

    Uses scipy's STFT with no implicit padding (boundary=None, padded=False)
    so frame timing is exact and reproducible: frame i covers samples
    [i*hop_size, i*hop_size + fft_size).
    """
    if audio.samples.size < config.fft_size:
        # Shorter than one full window: pad with zeros so we still get one
        # frame rather than raising — a valid (if low-information) result
        # for very short query clips.
        padded = np.zeros(config.fft_size, dtype=np.float32)
        padded[: audio.samples.size] = audio.samples
        signal = padded
    else:
        signal = audio.samples

    noverlap = config.fft_size - config.hop_size
    _freqs, _times, stft_matrix = stft(
        signal,
        fs=config.sample_rate,
        window=config.window,
        nperseg=config.fft_size,
        noverlap=noverlap,
        nfft=config.fft_size,
        boundary=None,
        padded=False,
        return_onesided=True,
    )

    magnitude = np.abs(stft_matrix)

    low_bin = config.min_freq_bin
    high_bin = min(config.max_freq_bin, magnitude.shape[0] - 1)
    band = magnitude[low_bin : high_bin + 1, :]

    magnitude_db = 20.0 * np.log10(np.maximum(band, _MAGNITUDE_EPSILON))

    return Spectrogram(
        magnitude_db=magnitude_db.astype(np.float32),
        bin_offset=low_bin,
        hop_size=config.hop_size,
        sample_rate=config.sample_rate,
    )
