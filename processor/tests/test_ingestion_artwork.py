from __future__ import annotations

from pathlib import Path

import numpy as np
import soundfile as sf
from mutagen.flac import FLAC, Picture

from music_fingerprint.ingestion import extract_artwork_bytes


def _tiny_png_bytes() -> bytes:
    # Smallest possible valid PNG (1x1 transparent pixel) — enough to
    # round-trip through mutagen without needing a real image file.
    return bytes.fromhex(
        "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4"
        "890000000a49444154789c6300010000050001a5f645400000000049454e44ae"
        "426082"
    )


def _flac_with_artwork(path: Path, sample_rate: int = 11025) -> None:
    samples = (0.1 * np.sin(2 * np.pi * 440 * np.arange(sample_rate) / sample_rate)).astype(
        np.float32
    )
    sf.write(path, samples, sample_rate, format="FLAC")

    audio = FLAC(path)
    picture = Picture()
    picture.type = 3  # front cover
    picture.mime = "image/png"
    picture.data = _tiny_png_bytes()
    audio.add_picture(picture)
    audio.save()


def test_extract_artwork_from_flac_picture(tmp_path):
    flac_path = tmp_path / "with_art.flac"
    _flac_with_artwork(flac_path)

    result = extract_artwork_bytes(flac_path)

    assert result is not None
    image_bytes, ext = result
    assert ext == ".png"
    assert image_bytes == _tiny_png_bytes()


def test_extract_artwork_returns_none_when_absent(tmp_path):
    flac_path = tmp_path / "no_art.flac"
    samples = np.zeros(11025, dtype=np.float32)
    sf.write(flac_path, samples, 11025, format="FLAC")

    assert extract_artwork_bytes(flac_path) is None


def test_extract_artwork_returns_none_for_garbage_file(tmp_path):
    garbage = tmp_path / "not_audio.flac"
    garbage.write_bytes(b"not a real flac file at all")

    assert extract_artwork_bytes(garbage) is None
