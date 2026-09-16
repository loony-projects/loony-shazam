use serde::Serialize;

use super::fingerprint::AlgorithmVersion;
use super::song::Song;

/// Why a query did not produce a confident match. The public API only ever
/// exposes `"NO_MATCH"` (see docs/api.md) — these finer-grained variants
/// exist for internal logging/metrics so operators can tell "nobody's
/// database had this song" apart from "matched, but ambiguously" without
/// changing the public contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecognitionReason {
    /// The processor extracted zero fingerprints from the query (e.g. near-silence).
    NoQueryFingerprints,
    /// No reference fingerprints matched any query hash at all.
    NoCandidates,
    /// A candidate existed but its dominant-offset vote count was too low.
    InsufficientVotes,
    /// A candidate existed but its composite score was too low.
    BelowScoreThreshold,
    /// A candidate existed but covered too little of the query's duration.
    BelowCoverageThreshold,
    /// The best and second-best candidates were too close to call.
    AmbiguousMargin,
}

impl RecognitionReason {
    pub fn as_api_str(&self) -> &'static str {
        "NO_MATCH"
    }

    pub fn as_metric_label(&self) -> &'static str {
        match self {
            RecognitionReason::NoQueryFingerprints => "no_query_fingerprints",
            RecognitionReason::NoCandidates => "no_candidates",
            RecognitionReason::InsufficientVotes => "insufficient_votes",
            RecognitionReason::BelowScoreThreshold => "below_score_threshold",
            RecognitionReason::BelowCoverageThreshold => "below_coverage_threshold",
            RecognitionReason::AmbiguousMargin => "ambiguous_margin",
        }
    }
}

/// The full result of a recognition attempt, independent of how it's
/// serialized for the HTTP response (see handlers/recognition_handler.rs).
#[derive(Debug, Clone)]
pub struct RecognitionOutcome {
    /// Set once the outcome has been persisted to the `recognitions` audit
    /// log (see `RecognitionService::record_audit`) — `None` briefly
    /// between the matching engine returning a result and the audit write
    /// completing, and if the audit write itself fails (never blocks the
    /// response — see recognition_service.rs).
    pub recognition_id: Option<i64>,
    pub song: Option<Song>,
    pub reason: Option<RecognitionReason>,
    pub score: f64,
    pub confidence: f64,
    pub matched_fingerprints: u32,
    pub query_fingerprints: u32,
    pub offset_ms: Option<i64>,
    pub algorithm_version: AlgorithmVersion,
    pub latency_ms: u64,
}

impl RecognitionOutcome {
    pub fn recognized(&self) -> bool {
        self.song.is_some()
    }
}
