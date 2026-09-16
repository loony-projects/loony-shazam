//! Turns raw vote histograms into a ranked, scored candidate list.
//!
//! `score` is NOT a calibrated probability. It is a ranking heuristic that
//! rewards both the raw number of temporally-consistent votes and how
//! *concentrated* those votes are in a single offset bucket relative to
//! total votes for that song (a real match's votes cluster tightly at one
//! offset; a coincidental partial match's votes spread across many
//! offsets). `confidence` is a bounded [0,1] heuristic derived from it for
//! display purposes — see docs/fingerprinting.md "Scoring" for the exact
//! derivation and its limits. Neither value should be presented to end
//! users as a statistically calibrated probability without a proper
//! calibration study against real-world query traffic.

use crate::models::SongId;

use super::voting::SongCandidate;

#[derive(Debug, Clone)]
pub struct CandidateScore {
    pub song_id: SongId,
    pub dominant_votes: u32,
    pub total_votes: u32,
    pub score: f64,
    pub confidence: f64,
    pub coverage: f64,
    pub matched_fingerprints: u32,
}

pub fn score_candidates(
    candidates: &std::collections::HashMap<SongId, SongCandidate>,
    query_fingerprint_count: u32,
) -> Vec<CandidateScore> {
    let mut scored: Vec<CandidateScore> = candidates
        .values()
        .filter_map(|c| {
            let song_id = c.song_id?;
            let (_, dominant_votes) = c.dominant_bucket();
            let total_votes = c.total_votes();
            if total_votes == 0 {
                return None;
            }

            // Concentration: what fraction of this song's total votes fell
            // in its single dominant bucket. 1.0 = every vote agrees on one
            // offset (strong signal); low values = votes are scattered
            // (weak/coincidental signal).
            let concentration = dominant_votes as f64 / total_votes as f64;

            // Coverage: fraction of the query's own landmarks that were
            // explained by this song at its dominant offset specifically
            // (not just "matched somewhere").
            let coverage = if query_fingerprint_count > 0 {
                c.dominant_bucket_unique_hashes() as f64 / query_fingerprint_count as f64
            } else {
                0.0
            };

            let score = dominant_votes as f64 * concentration;

            // Confidence: bounded [0,1] view of "how much of the query is
            // explained by one consistent offset in this song". Documented
            // as a heuristic, not a calibrated probability (see module doc).
            let confidence =
                (dominant_votes as f64 / query_fingerprint_count.max(1) as f64).clamp(0.0, 1.0);

            Some(CandidateScore {
                song_id,
                dominant_votes,
                total_votes,
                score,
                confidence,
                coverage,
                matched_fingerprints: c.all_matched_unique_hashes() as u32,
            })
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::index::FingerprintMatch;
    use crate::matching::voting::{build_candidates, QueryFingerprint};
    use crate::models::{FingerprintHash, OffsetMs};

    #[test]
    fn concentrated_votes_outscore_scattered_votes_with_same_total() {
        // Song A: 5 votes, all in the same bucket (concentration = 1.0)
        let query_a = vec![
            QueryFingerprint {
                hash: FingerprintHash(1),
                offset_ms: OffsetMs(0),
            },
            QueryFingerprint {
                hash: FingerprintHash(2),
                offset_ms: OffsetMs(0),
            },
            QueryFingerprint {
                hash: FingerprintHash(3),
                offset_ms: OffsetMs(0),
            },
            QueryFingerprint {
                hash: FingerprintHash(4),
                offset_ms: OffsetMs(0),
            },
            QueryFingerprint {
                hash: FingerprintHash(5),
                offset_ms: OffsetMs(0),
            },
        ];
        let matches_a: Vec<_> = (1..=5)
            .map(|h| FingerprintMatch {
                hash: FingerprintHash(h),
                song_id: SongId(1),
                offset_ms: OffsetMs(1000),
            })
            .collect();

        // Song B: 5 votes, scattered across 5 different buckets
        let matches_b: Vec<_> = (1..=5)
            .map(|h| FingerprintMatch {
                hash: FingerprintHash(h),
                song_id: SongId(2),
                offset_ms: OffsetMs((h as i32) * 1000),
            })
            .collect();

        let mut all_matches = matches_a;
        all_matches.extend(matches_b);

        let candidates = build_candidates(&query_a, &all_matches, 100);
        let scored = score_candidates(&candidates, 5);

        let song_a_score = scored.iter().find(|s| s.song_id == SongId(1)).unwrap();
        let song_b_score = scored.iter().find(|s| s.song_id == SongId(2)).unwrap();

        assert!(song_a_score.score > song_b_score.score);
        assert_eq!(song_a_score.dominant_votes, 5);
        assert_eq!(song_b_score.dominant_votes, 1);
    }
}
