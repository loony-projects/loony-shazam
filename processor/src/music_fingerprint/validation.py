"""Shared exceptions and pure validation helpers.

Audio arriving at this service — whether from an HTTP upload or a file on
disk during ingestion — is treated as untrusted input. We never trust a
claimed MIME type, filename extension, or embedded duration metadata; we
only trust what the decoder actually produces.
"""

from __future__ import annotations


class AudioValidationError(Exception):
    """Base class for all audio-input problems (never a bug in our code)."""


class AudioTooLargeError(AudioValidationError):
    def __init__(self, size_bytes: int, max_bytes: int) -> None:
        super().__init__(f"audio payload is {size_bytes} bytes, exceeds limit of {max_bytes} bytes")
        self.size_bytes = size_bytes
        self.max_bytes = max_bytes


class AudioTooLongError(AudioValidationError):
    def __init__(self, duration_seconds: float, max_seconds: float) -> None:
        super().__init__(
            f"decoded audio is {duration_seconds:.2f}s, exceeds limit of {max_seconds:.2f}s"
        )
        self.duration_seconds = duration_seconds
        self.max_seconds = max_seconds


class AudioEmptyError(AudioValidationError):
    def __init__(self) -> None:
        super().__init__("decoded audio has zero samples")


class AudioDecodeError(AudioValidationError):
    def __init__(self, reason: str) -> None:
        super().__init__(f"could not decode audio: {reason}")


def validate_payload_size(size_bytes: int, max_bytes: int) -> None:
    if size_bytes <= 0:
        raise AudioEmptyError()
    if size_bytes > max_bytes:
        raise AudioTooLargeError(size_bytes, max_bytes)


def validate_decoded_duration(num_samples: int, sample_rate: int, max_seconds: float) -> float:
    if num_samples <= 0:
        raise AudioEmptyError()
    duration_seconds = num_samples / sample_rate
    if duration_seconds > max_seconds:
        raise AudioTooLongError(duration_seconds, max_seconds)
    return duration_seconds
