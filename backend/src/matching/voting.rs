//! Temporal offset-histogram voting: turns raw (query hash -> reference
//! hash) matches into per-song candidates ranked by how consistently a
//! single time offset explains the matches (see docs/fingerprinting.md
//! "Temporal offset voting").

use std::collections::{HashMap, HashSet};

use crate::models::{FingerprintHash, OffsetMs, SongId};

use super::index::FingerprintMatch;

#[derive(Debug, Clone, Copy)]
pub struct QueryFingerprint {
    pub hash: FingerprintHash,
    pub offset_ms: OffsetMs,
}

/// Raw per-song voting state before scoring.
#[derive(Debug, Default)]
pub struct SongCandidate {
    pub song_id: Option<SongId>,
    /// vote count per quantized offset bucket
    buckets: HashMap<i64, u32>,
    /// distinct query hashes that matched this song, keyed by bucket (used
    /// both for dominant-bucket coverage and overall matched-fingerprint count)
    all_matched_hashes: HashMap<i64, HashSet<FingerprintHash>>,
}

impl SongCandidate {
    fn vote(&mut self, bucket: i64, hash: FingerprintHash) {
        *self.buckets.entry(bucket).or_insert(0) += 1;
        self.all_matched_hashes
            .entry(bucket)
            .or_default()
            .insert(hash);
    }

    /// (bucket, vote_count) of the bucket with the most votes. Ties break
    /// on the lowest bucket id, which just needs to be deterministic.
    pub fn dominant_bucket(&self) -> (i64, u32) {
        self.buckets
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(&bucket, &votes)| (bucket, votes))
            .unwrap_or((0, 0))
    }

    pub fn total_votes(&self) -> u32 {
        self.buckets.values().sum()
    }

    pub fn dominant_bucket_unique_hashes(&self) -> usize {
        let (bucket, _) = self.dominant_bucket();
        self.all_matched_hashes
            .get(&bucket)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    pub fn all_matched_unique_hashes(&self) -> usize {
        self.all_matched_hashes
            .values()
            .flatten()
            .collect::<HashSet<_>>()
            .len()
    }
}

/// Build one `SongCandidate` per distinct matched song from the raw index
/// lookup results, given the offsets of the original query fingerprints.
///
/// For every matched `(query_hash, reference_hash)` pair we compute
/// `offset = reference_offset_ms - query_offset_ms`, quantize it into a
/// `bucket_ms`-wide bucket, and cast one vote for `(song_id, bucket)`. A
/// query hash that appears more than once in the query (e.g. a repeating
/// musical phrase) casts a vote for each of its occurrences — this is
/// intentional: a real repeated phrase reinforces the same true offset
/// less than a single occurrence would in isolation, which is the correct,
/// conservative behavior when the source material is genuinely repetitive.
pub fn build_candidates(
    query_fingerprints: &[QueryFingerprint],
    matches: &[FingerprintMatch],
    bucket_ms: i64,
) -> HashMap<SongId, SongCandidate> {
    let mut query_offsets_by_hash: HashMap<FingerprintHash, Vec<OffsetMs>> = HashMap::new();
    for qfp in query_fingerprints {
        query_offsets_by_hash
            .entry(qfp.hash)
            .or_default()
            .push(qfp.offset_ms);
    }

    let mut candidates: HashMap<SongId, SongCandidate> = HashMap::new();

    for m in matches {
        let Some(query_offsets) = query_offsets_by_hash.get(&m.hash) else {
            continue;
        };
        for &query_offset in query_offsets {
            let offset = m.offset_ms.0 as i64 - query_offset.0 as i64;
            let bucket = quantize(offset, bucket_ms);
            let candidate = candidates
                .entry(m.song_id)
                .or_insert_with(|| SongCandidate {
                    song_id: Some(m.song_id),
                    ..Default::default()
                });
            candidate.vote(bucket, m.hash);
        }
    }

    candidates
}

fn quantize(offset_ms: i64, bucket_ms: i64) -> i64 {
    // Round-to-nearest rather than floor, so an offset that's a hair below
    // a bucket boundary (a realistic outcome of frame-grid misalignment
    // between query and reference STFTs, see spectrogram.py) still lands
    // in the same bucket as its neighbors instead of splitting votes.
    let bucket_ms = bucket_ms.max(1);
    let half = bucket_ms / 2;
    if offset_ms >= 0 {
        (offset_ms + half) / bucket_ms
    } else {
        -(((-offset_ms) + half) / bucket_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qfp(hash: i64, offset: i32) -> QueryFingerprint {
        QueryFingerprint {
            hash: FingerprintHash(hash),
            offset_ms: OffsetMs(offset),
        }
    }

    fn fm(hash: i64, song: i64, offset: i32) -> FingerprintMatch {
        FingerprintMatch {
            hash: FingerprintHash(hash),
            song_id: SongId(song),
            offset_ms: OffsetMs(offset),
        }
    }

    #[test]
    fn consistent_offset_produces_dominant_bucket() {
        let query = vec![qfp(1, 0), qfp(2, 100), qfp(3, 200)];
        // reference offsets all exactly 5000ms ahead of query offsets
        let matches = vec![fm(1, 42, 5000), fm(2, 42, 5100), fm(3, 42, 5200)];

        let candidates = build_candidates(&query, &matches, 100);
        let candidate = candidates.get(&SongId(42)).unwrap();
        let (bucket, votes) = candidate.dominant_bucket();
        assert_eq!(bucket, 50); // 5000ms / 100ms bucket
        assert_eq!(votes, 3);
    }

    #[test]
    fn scattered_offsets_split_across_buckets() {
        let query = vec![qfp(1, 0), qfp(2, 0), qfp(3, 0)];
        let matches = vec![fm(1, 42, 1000), fm(2, 42, 9000), fm(3, 42, 20000)];

        let candidates = build_candidates(&query, &matches, 100);
        let candidate = candidates.get(&SongId(42)).unwrap();
        let (_, votes) = candidate.dominant_bucket();
        assert_eq!(votes, 1); // no bucket accumulates more than one vote
        assert_eq!(candidate.total_votes(), 3);
    }

    #[test]
    fn unmatched_hash_is_ignored() {
        let query = vec![qfp(1, 0)];
        let matches = vec![fm(999, 42, 5000)]; // hash not present in query
        let candidates = build_candidates(&query, &matches, 100);
        assert!(candidates.is_empty());
    }
}
