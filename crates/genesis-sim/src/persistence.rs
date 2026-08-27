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
    #[error("Snapshot is corrupt: {0}")]
    Corrupt(String),
}

/// 圧縮スナップショットの中身（config, heightfield, causality, settlements）。
type SnapshotSections = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

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

    /// スナップショットが読み戻せることを確かめる。壊れている場合は
    /// `false` ではなく理由付きのエラーを返す（原因を失わないため）。
    pub fn verify_snapshot_integrity(path: impl AsRef<Path>) -> Result<bool, PersistenceError> {
        let file = File::open(path)?;
        let mut decoder = zstd::stream::Decoder::new(file)?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        if decompressed.is_empty() {
            return Err(PersistenceError::Corrupt(
                "decompressed payload is empty".to_string(),
            ));
        }
        // 中身が本当に4区画のスナップショットとして読めるかまで検査する。
        bincode::deserialize::<SnapshotSections>(&decompressed)
            .map_err(|e| PersistenceError::Corrupt(e.to_string()))?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldGenesisConfig;

    fn small_world(seed: u64) -> WorldSimulation {
        WorldSimulation::new(WorldGenesisConfig {
            seed,
            map_width: 16,
            map_height: 16,
            plate_count: 4,
            sea_level: 0.0,
            solar_luminosity: 1.0,
            axial_tilt_deg: 23.44,
        })
    }

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("wg_snapshot_{tag}_{}.bin.zst", std::process::id()))
    }

    #[test]
    fn a_valid_snapshot_verifies() {
        let path = temp_path("valid");
        let world = small_world(0xC0FF_EE01);
        WorldSnapshotService::save_world_compressed(&world, &path).unwrap();
        assert!(WorldSnapshotService::verify_snapshot_integrity(&path).unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_truncated_snapshot_reports_corruption() {
        let path = temp_path("corrupt");
        let world = small_world(0xC0FF_EE02);
        WorldSnapshotService::save_world_compressed(&world, &path).unwrap();

        // 圧縮そのものは正しいが、中身が途中で切れている状態を作る。
        let mut buffer = Vec::new();
        {
            let file = File::open(&path).unwrap();
            zstd::stream::Decoder::new(file)
                .unwrap()
                .read_to_end(&mut buffer)
                .unwrap();
        }
        buffer.truncate(buffer.len() / 2);
        {
            let file = File::create(&path).unwrap();
            let mut encoder = zstd::stream::Encoder::new(file, 3).unwrap();
            encoder.write_all(&buffer).unwrap();
            encoder.finish().unwrap();
        }

        let result = WorldSnapshotService::verify_snapshot_integrity(&path);
        assert!(matches!(result, Err(PersistenceError::Corrupt(_))), "{result:?}");
        let _ = std::fs::remove_file(&path);
    }
}
