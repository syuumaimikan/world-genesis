use crate::world::WorldSimulation;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("I/O failure during save/load: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bincode serialization failed: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
}

/// 展開後のスナップショットとして受け入れる上限（512 MiB）。
/// zstd は極小の入力から巨大な出力を作れるため、上限なしに展開しない。
pub const MAX_UNCOMPRESSED_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;

pub struct WorldSnapshotService;

impl WorldSnapshotService {
    pub fn save_world_compressed(world: &WorldSimulation, path: impl AsRef<Path>) -> Result<(), PersistenceError> {
        let serialized_config = bincode::serialize(&world.config)?;
        let serialized_heightfield = bincode::serialize(&world.heightfield)?;
        let serialized_causality = bincode::serialize(&world.causality)?;
        let serialized_settlements = bincode::serialize(&world.settlements)?;

        let mut buffer = Vec::new();
        bincode::serialize_into(
            &mut buffer,
            &(&serialized_config, &serialized_heightfield, &serialized_causality, &serialized_settlements),
        )?;

        let file = File::create(path)?;
        let mut encoder = zstd::stream::Encoder::new(file, 3)?;
        encoder.write_all(&buffer)?;
        encoder.finish()?;

        Ok(())
    }

    pub fn verify_snapshot_integrity(path: impl AsRef<Path>) -> Result<bool, PersistenceError> {
        let file = File::open(path)?;
        let decoder = zstd::stream::Decoder::new(file)?;
        let mut decompressed = Vec::new();
        decoder
            .take(MAX_UNCOMPRESSED_SNAPSHOT_BYTES)
            .read_to_end(&mut decompressed)?;
        Ok(!decompressed.is_empty())
    }
}
