//! Orchestrates a full recognition: lookup -> vote -> score -> threshold ->
//! resolve song metadata. See docs/architecture.md "Matching engine" and
//! docs/fingerprinting.md for the algorithm this implements.

use std::sync::Arc;

use crate::config::MatchingConfig;
use crate::models::{AlgorithmVersion, RecognitionOutcome, RecognitionReason};
use crate::repositories::SongRepository;

use super::index::FingerprintIndex;
use super::scoring::score_candidates;
use super::voting::{build_candidates, QueryFingerprint};

pub struct MatchingEngine {
    index: Arc<dyn FingerprintIndex>,
    songs: SongRepository,
    config: MatchingConfig,
}

impl MatchingEngine {
    pub fn new(
        index: Arc<dyn FingerprintIndex>,
        songs: SongRepository,
        config: MatchingConfig,
    ) -> Self {
        Self {
            index,
            songs,
            config,
        }
    }

    pub async fn recognize(
        &self,
        query_fingerprints: Vec<QueryFingerprint>,
        algorithm_version: AlgorithmVersion,
        started: std::time::Instant,
    ) -> Result<RecognitionOutcome, crate::error::AppError> {
        let query_count = query_fingerprints.len() as u32;

        if query_fingerprints.is_empty() {
            return Ok(self.no_match(
                RecognitionReason::NoQueryFingerprints,
                algorithm_version,
                0,
                started,
            ));
        }

        let hashes: Vec<_> = query_fingerprints.iter().map(|f| f.hash).collect();
        let matches = self.index.lookup(&hashes, algorithm_version).await?;

        if matches.is_empty() {
            return Ok(self.no_match(
                RecognitionReason::NoCandidates,
                algorithm_version,
                query_count,
                started,
            ));
        }

        let candidates =
            build_candidates(&query_fingerprints, &matches, self.config.offset_bucket_ms);
        let scored = score_candidates(&candidates, query_count);

        if let Some(top) = scored.first() {
            tracing::debug!(
                song_id = top.song_id.0,
                dominant_votes = top.dominant_votes,
                total_votes = top.total_votes,
                score = top.score,
                coverage = top.coverage,
                num_candidates = scored.len(),
                "top recognition candidate"
            );
        }

        let Some(best) = scored.first() else {
            return Ok(self.no_match(
                RecognitionReason::NoCandidates,
                algorithm_version,
                query_count,
                started,
            ));
        };

        if best.dominant_votes < self.config.min_dominant_votes {
            return Ok(self.no_match(
                RecognitionReason::InsufficientVotes,
                algorithm_version,
                query_count,
                started,
            ));
        }
        if best.score < self.config.min_score {
            return Ok(self.no_match(
                RecognitionReason::BelowScoreThreshold,
                algorithm_version,
                query_count,
                started,
            ));
        }
        if best.coverage < self.config.min_coverage {
            return Ok(self.no_match(
                RecognitionReason::BelowCoverageThreshold,
                algorithm_version,
                query_count,
                started,
            ));
        }
        if let Some(second) = scored.get(1) {
            let margin_ok = second.dominant_votes == 0
                || (best.dominant_votes as f64 / second.dominant_votes as f64)
                    >= self.config.min_margin_ratio;
            if !margin_ok {
                return Ok(self.no_match(
                    RecognitionReason::AmbiguousMargin,
                    algorithm_version,
                    query_count,
                    started,
                ));
            }
        }

        let song = self.songs.find_by_id(best.song_id).await?;
        let Some(song) = song else {
            // Fingerprints exist for a song_id that no longer has a songs
            // row (e.g. deleted out from under a stale index) — treat as
            // no match rather than surfacing a dangling reference.
            return Ok(self.no_match(
                RecognitionReason::NoCandidates,
                algorithm_version,
                query_count,
                started,
            ));
        };

        let candidate = candidates
            .get(&best.song_id)
            .expect("scored candidate must exist");
        let (dominant_bucket, _) = candidate.dominant_bucket();

        Ok(RecognitionOutcome {
            recognition_id: None,
            song: Some(song),
            reason: None,
            score: best.score,
            confidence: best.confidence,
            matched_fingerprints: best.matched_fingerprints,
            query_fingerprints: query_count,
            offset_ms: Some(dominant_bucket * self.config.offset_bucket_ms),
            algorithm_version,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn no_match(
        &self,
        reason: RecognitionReason,
        algorithm_version: AlgorithmVersion,
        query_count: u32,
        started: std::time::Instant,
    ) -> RecognitionOutcome {
        RecognitionOutcome {
            recognition_id: None,
            song: None,
            reason: Some(reason),
            score: 0.0,
            confidence: 0.0,
            matched_fingerprints: 0,
            query_fingerprints: query_count,
            offset_ms: None,
            algorithm_version,
            latency_ms: started.elapsed().as_millis() as u64,
        }
    }
}
