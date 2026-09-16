use serde::{Deserialize, Serialize};

/// A packed landmark hash as produced by the Python reference
/// implementation (see docs/fingerprinting.md for the bit layout). Stored
/// as `BIGINT` in Postgres; kept as `i64` here even though the packed
/// value only ever uses the low 32 bits, leaving headroom to widen the
/// hash format later without a schema change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct FingerprintHash(pub i64);

/// Offset of a landmark from the start of its track, in milliseconds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct OffsetMs(pub i32);

/// Fingerprint algorithm version. Every fingerprint row and every
/// recognition response carries one, so incompatible algorithm revisions
/// can never be silently compared (see docs/fingerprinting.md "Algorithm
/// versioning").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct AlgorithmVersion(pub i16);
