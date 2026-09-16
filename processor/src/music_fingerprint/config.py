"""Central, immutable configuration for the fingerprinting pipeline.

Every tunable used anywhere in `music_fingerprint` lives here so the
algorithm has exactly one source of truth for its parameters. Nothing in
this package should hard-code a sample rate, FFT size, or threshold outside
of this module.
"""

from __future__ import annotations

from dataclasses import dataclass

#: Bumped whenever a change to this module or the algorithm in
#: `fingerprint.py`/`hashing.py` would produce different hashes for the same
#: input audio. Encoded into the top 4 bits of every hash (see hashing.py)
#: *and* stored as a separate column, so old and new fingerprints can never
#: silently collide or be compared against each other.
FINGERPRINT_ALGORITHM_VERSION = 1


@dataclass(frozen=True, slots=True)
class FingerprintConfig:
    # --- Audio normalization -------------------------------------------
    # 11025 Hz (a quarter of the common 44.1kHz consumer rate) keeps the
    # Nyquist frequency at ~5.5kHz, comfortably above our analysis band
    # (40-5000 Hz) where the vast majority of musically-distinctive,
    # perceptually stable spectral energy lives. This is the same rate
    # used by the original Shazam paper (Wang, 2003) and halves FFT cost
    # relative to 22050 Hz with no measurable recognition-quality loss for
    # music (as opposed to speech, which needs more high-frequency detail).
    sample_rate: int = 11025

    # --- STFT -------------------------------------------------------------
    # 4096 samples @ 11025 Hz = ~371 ms window: long enough to resolve
    # musically meaningful frequency detail (bin width ~2.69 Hz) while
    # staying short enough that transient landmarks (onsets) are still
    # time-localized.
    fft_size: int = 4096
    # 512 samples @ 11025 Hz = ~46.4 ms hop (87.5% overlap): gives enough
    # time resolution that landmark timing survives +/-50ms of playback
    # jitter/mic latency without the peak's time bin shifting by more than
    # one bucket.
    hop_size: int = 512
    window: str = "hann"

    # --- Frequency band of interest ---------------------------------------
    # Below 40 Hz is sub-bass rarely present/distinctive in consumer audio
    # (and often just DC/rumble); above 5000 Hz consumer playback/mic
    # chains (phone speakers, laptop mics) roll off heavily and MP3
    # encoding at typical bitrates discards it first, so peaks up there are
    # unstable across "real-world" re-recordings.
    min_frequency_hz: float = 40.0
    max_frequency_hz: float = 5000.0

    # --- Peak detection -----------------------------------------------
    # Local-maximum neighborhood, in (freq_bins, time_bins). A peak must be
    # the strongest bin within this window to be kept. Asymmetric on
    # purpose: a narrower frequency window (9 bins, ~24 Hz) preserves
    # resolution between musically-close pitches, while a wider time
    # window (25 frames, ~1.16s) suppresses redundant near-duplicate peaks
    # from a single sustained note. Values were chosen empirically (see
    # docs/performance.md) by sweeping neighborhood size and the dB
    # threshold below against synthetic reference/noisy-query pairs and
    # picking the combination that kept a >5x vote margin between the
    # correct offset bucket and the next-best bucket up to noise added at
    # 0.3x the (peak-normalized) signal's RMS-equivalent amplitude.
    peak_neighborhood_time: int = 25
    peak_neighborhood_freq: int = 9
    # A peak must exceed the mean magnitude of its local neighborhood by
    # at least this many dB to be considered "strong" rather than noise.
    # 6dB (the original conservative guess) let noise-floor bins compete
    # with real peaks once moderate noise was added, roughly 7x inflating
    # peak/fingerprint counts and collapsing the offset-histogram vote
    # margin to near zero (see docs/performance.md for the measured sweep).
    # 14dB keeps peak count stable under noise and preserves a clear
    # dominant-offset margin.
    peak_min_db_above_local_mean: float = 14.0
    # Hard floor in dB (relative to the spectrogram's own max) below which
    # nothing is ever considered a peak, regardless of local contrast. Keeps
    # near-silence from producing peaks out of numerical noise.
    peak_absolute_floor_db: float = -60.0
    # Upper bound on peaks kept per second of audio. Protects against
    # pathological density (e.g. white noise, clipping) generating an
    # unbounded number of fingerprints. Peaks are ranked by amplitude and
    # thinned to this budget if exceeded.
    max_peaks_per_second: int = 60

    # --- Landmark / target-zone pairing -----------------------------------
    # For each anchor peak, pair with up to `fanout` subsequent peaks whose
    # time offset from the anchor falls in
    # [target_zone_min_frames, target_zone_max_frames]. min=1 avoids
    # degenerate dt=0 pairs; max=100 frames (~4.6s at the default hop)
    # keeps delta_t comfortably inside its 8-bit hash field (max 255) while
    # covering enough temporal context for distinctive pairs.
    fanout: int = 5
    target_zone_min_frames: int = 1
    target_zone_max_frames: int = 100

    # --- Hashing ------------------------------------------------------
    # Bit widths for the packed 32-bit hash (see hashing.py for the exact
    # layout). anchor/target frequency fields are 10 bits each (0-1023);
    # delta_t is 8 bits (0-255, matching target_zone_max_frames above).
    freq_bits: int = 10
    delta_bits: int = 8
    version_bits: int = 4

    # --- Safety limits ------------------------------------------------
    # Reject/truncate audio outside these bounds before doing any DSP work.
    # A 12-second cap matches the Android client's max capture duration;
    # ingestion tracks can be much longer, so a generous ceiling here still
    # protects against adversarially huge uploads on the *query* path
    # (the ingestion CLI applies its own, larger limit — see ingestion.py).
    max_query_duration_seconds: float = 15.0
    max_ingest_duration_seconds: float = 3600.0

    @property
    def hop_seconds(self) -> float:
        return self.hop_size / self.sample_rate

    @property
    def freq_bin_hz(self) -> float:
        return self.sample_rate / self.fft_size

    @property
    def min_freq_bin(self) -> int:
        return max(1, int(self.min_frequency_hz / self.freq_bin_hz))

    @property
    def max_freq_bin(self) -> int:
        nyquist_bin = self.fft_size // 2
        return min(nyquist_bin, int(self.max_frequency_hz / self.freq_bin_hz) + 1)


#: The single default configuration used by the processor service and the
#: ingestion CLI. Passing a different config anywhere is supported (tests
#: use tiny/fast variants) but production paths always use this instance.
DEFAULT_CONFIG = FingerprintConfig()
