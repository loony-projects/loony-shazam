"""Long-lived FastAPI extraction service.

The Rust backend never spawns a Python process per request; instead it
talks to this persistent service over HTTP so process/import/JIT warm-up
cost is paid once, not per recognition. See docs/architecture.md
"Processor service internal protocol".
"""

from __future__ import annotations

import base64
import time

import structlog
from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field

from music_fingerprint.config import (
    DEFAULT_CONFIG,
    FINGERPRINT_ALGORITHM_VERSION,
    FingerprintConfig,
)
from music_fingerprint.extraction import DEFAULT_MAX_QUERY_BYTES, extract_from_bytes
from music_fingerprint.validation import AudioValidationError

log = structlog.get_logger()

app = FastAPI(title="music-fingerprint processor", version="1.0.0")


class ExtractRequest(BaseModel):
    audio_base64: str = Field(..., description="Base64-encoded audio bytes")
    content_type: str | None = None
    filename_hint: str | None = None
    max_duration_seconds: float | None = Field(
        default=None,
        description="Overrides config.max_query_duration_seconds; lets the "
        "backend enforce its own MAX_AUDIO_DURATION_SECONDS without a code change here.",
    )


class FingerprintOut(BaseModel):
    hash: int
    offset_ms: int


class ExtractResponse(BaseModel):
    algorithm_version: int
    sample_rate: int
    duration_ms: int
    fingerprints: list[FingerprintOut]


class ErrorResponse(BaseModel):
    error: str
    detail: str


@app.middleware("http")
async def log_requests(request: Request, call_next):
    start = time.monotonic()
    response = await call_next(request)
    elapsed_ms = (time.monotonic() - start) * 1000
    log.info(
        "request",
        path=request.url.path,
        method=request.method,
        status=response.status_code,
        elapsed_ms=round(elapsed_ms, 2),
    )
    return response


@app.get("/internal/v1/health")
def health() -> dict:
    return {"status": "ok"}


@app.get("/internal/v1/algorithm")
def algorithm_info() -> dict:
    config: FingerprintConfig = DEFAULT_CONFIG
    return {
        "algorithm_version": FINGERPRINT_ALGORITHM_VERSION,
        "sample_rate": config.sample_rate,
        "fft_size": config.fft_size,
        "hop_size": config.hop_size,
    }


@app.post("/internal/v1/extract", response_model=ExtractResponse)
def extract(req: ExtractRequest) -> ExtractResponse:
    try:
        data = base64.b64decode(req.audio_base64, validate=True)
    except Exception as exc:  # noqa: BLE001 - any base64 failure is a client error
        raise HTTPException(status_code=400, detail=f"invalid base64 audio payload: {exc}") from exc

    try:
        result = extract_from_bytes(
            data,
            config=DEFAULT_CONFIG,
            max_bytes=DEFAULT_MAX_QUERY_BYTES,
            max_duration_seconds=req.max_duration_seconds,
            filename_hint=req.filename_hint,
        )
    except AudioValidationError as exc:
        log.warning("extraction_rejected", reason=str(exc))
        raise HTTPException(status_code=422, detail=str(exc)) from exc

    log.info(
        "extraction_ok",
        duration_ms=result.duration_ms,
        num_fingerprints=len(result.fingerprints),
    )

    return ExtractResponse(
        algorithm_version=result.algorithm_version,
        sample_rate=result.sample_rate,
        duration_ms=result.duration_ms,
        fingerprints=[
            FingerprintOut(hash=f.hash, offset_ms=f.offset_ms) for f in result.fingerprints
        ],
    )


@app.exception_handler(AudioValidationError)
def handle_validation_error(_request: Request, exc: AudioValidationError) -> JSONResponse:
    return JSONResponse(
        status_code=422,
        content=ErrorResponse(error="INVALID_AUDIO", detail=str(exc)).model_dump(),
    )
