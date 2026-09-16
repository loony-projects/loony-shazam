"""Reference audio fingerprinting DSP pipeline (Shazam-style landmark hashing).

This package is the authoritative implementation of the fingerprinting
algorithm. The Rust backend never reimplements DSP; it only consumes the
fingerprints produced here (see docs/architecture.md).
"""

from music_fingerprint.config import DEFAULT_CONFIG, FingerprintConfig
from music_fingerprint.fingerprint import Fingerprint, generate_fingerprints

__all__ = [
    "FingerprintConfig",
    "DEFAULT_CONFIG",
    "Fingerprint",
    "generate_fingerprints",
]
