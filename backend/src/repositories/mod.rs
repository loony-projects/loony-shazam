mod fingerprint_repository;
mod recognition_repository;
mod song_repository;

pub use fingerprint_repository::FingerprintRepository;
pub use recognition_repository::{NewRecognitionRecord, RecognitionRecord, RecognitionRepository};
pub use song_repository::{NewSong, SongRepository};
