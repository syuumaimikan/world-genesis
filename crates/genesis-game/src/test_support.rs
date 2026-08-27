//! テストが世界やディレクトリを用意するための共通の足場。
//!
//! ほとんどのテストは「起伏も洞窟も植生も無い、完全に予測できる平坦な世界」を
//! 欲しがる。各モジュールが同じ組み立てを書き写すと、生成パラメータが増えた
//! ときに一部のテストだけ古い前提のまま残ってしまうため、ここへ集約する。

use crate::blocks::BlockRegistry;
use crate::chunk::{ChunkData, ChunkPos};
use crate::streaming::VoxelWorld;
use crate::worldgen::{GenParams, WorldGenerator};
use std::path::PathBuf;
use std::sync::Arc;

/// 地形の起伏・洞窟・植生・集落を全て切った生成パラメータ。
///
/// レイキャストや物理の期待値が地形の揺らぎに左右されないようにする。
pub fn flat_params() -> GenParams {
    GenParams {
        flat_world: true,
        cave_density: 0.0,
        vegetation_density: 0.0,
        settlement_density: 0.0,
        ..GenParams::default()
    }
}

/// 組み込みブロックだけを積んだ、チャンク未生成の世界。
pub fn world_with(params: GenParams, seed: u64) -> VoxelWorld {
    let lookup = BlockRegistry::with_builtins().snapshot();
    VoxelWorld::new(WorldGenerator::new(seed, params), lookup)
}

/// 平坦な世界を作り、原点周り `radius` チャンクを生成しておく。
pub fn flat_world(seed: u64, radius: i32) -> VoxelWorld {
    let mut w = world_with(flat_params(), seed);
    w.prime_chunks_around(ChunkPos::new(0, 0), radius);
    w
}

/// 空（全て空気）のチャンクを敷いた世界。自分でブロックを置いて試すとき用。
pub fn empty_world(seed: u64, radius: i32) -> VoxelWorld {
    let mut w = world_with(
        GenParams {
            flat_world: true,
            ..GenParams::default()
        },
        seed,
    );
    for cz in -radius..=radius {
        for cx in -radius..=radius {
            let p = ChunkPos::new(cx, cz);
            w.chunks.insert(p, Arc::new(ChunkData::empty(p)));
        }
    }
    w
}

/// テスト専用の作業ディレクトリ。プロセス ID と呼び出しごとの連番で、
/// 並行実行しても他のテストと衝突しない。
pub fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("wg_{tag}_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}
