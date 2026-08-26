//! セーブデータの作成・列挙・保存・読み込み・削除。
//!
//! ボクセル世界は無限に広がるため、全チャンクを保存することはできない。
//! ワールド生成が (シード, 座標) の純関数である性質を利用し、
//! **改変されたチャンクだけ** を差分として保存する。手つかずの土地は
//! 読み込み時に同じシードから再生成され、完全に同じ姿で戻ってくる。

use crate::chunk::{ChunkData, ChunkPos, PaletteRleChunk};
use crate::worldgen::GenParams;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// セーブ形式のバージョン。読み込み時に照合し、将来の移行処理の起点にする。
pub const SAVE_FORMAT_VERSION: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("入出力エラー: {0}")]
    Io(#[from] std::io::Error),
    #[error("セーブデータの解析に失敗: {0}")]
    Decode(String),
    #[error("セーブ形式 v{found} は非対応です（本体は v{expected}）")]
    Version { found: u32, expected: u32 },
    #[error("同名のワールド '{0}' が既に存在します")]
    AlreadyExists(String),
    #[error("ワールド '{0}' が見つかりません")]
    NotFound(String),
    #[error("ワールド名が不正です: {0}")]
    BadName(String),
}

/// ワールド作成画面で決まる、その世界の不変の素性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMeta {
    pub format_version: u32,
    pub display_name: String,
    /// ディレクトリ名（安全な文字へ正規化済み）。
    pub folder: String,
    pub seed: u64,
    pub world_type: WorldType,
    pub game_mode: GameMode,
    /// 生成パラメータ。
    pub sea_level: i32,
    pub terrain_amplitude: f32,
    pub cave_density: f32,
    pub ore_richness: f32,
    pub vegetation_density: f32,
    pub settlement_density: f32,
    /// このワールドで有効なプラグイン。整合性のためワールドごとに固定する。
    pub plugins: Vec<String>,
    /// 累積プレイ時間（秒）。
    pub played_seconds: f64,
    /// シミュレーション内の経過 tick。
    pub sim_tick: u64,
    pub created_unix: u64,
    pub last_played_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldType {
    /// 標準の大陸・海洋・山脈。
    Continents,
    /// 起伏を強調した山岳世界。
    Amplified,
    /// 島が点在する群島世界。
    Islands,
    /// 完全な平地（建築・デバッグ用）。
    Flat,
}

impl WorldType {
    pub fn display_name(self) -> &'static str {
        match self {
            WorldType::Continents => "標準（大陸と海）",
            WorldType::Amplified => "山岳強調",
            WorldType::Islands => "群島",
            WorldType::Flat => "フラット",
        }
    }

    pub const ALL: [WorldType; 4] = [
        WorldType::Continents,
        WorldType::Amplified,
        WorldType::Islands,
        WorldType::Flat,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    /// 一人の住人として生きる。
    Survival,
    /// 資源制限なしで建築する。
    Creative,
    /// 世界だけを観察する（仕様 78）。
    Observer,
}

impl GameMode {
    pub fn display_name(self) -> &'static str {
        match self {
            GameMode::Survival => "サバイバル（世界の住人として生きる）",
            GameMode::Creative => "クリエイティブ（自由建築）",
            GameMode::Observer => "オブザーバー（世界を観察する）",
        }
    }

    pub const ALL: [GameMode; 3] = [GameMode::Survival, GameMode::Creative, GameMode::Observer];
}

impl WorldMeta {
    pub fn to_gen_params(&self) -> GenParams {
        let mut p = GenParams {
            sea_level: self.sea_level,
            terrain_amplitude: self.terrain_amplitude,
            cave_density: self.cave_density,
            ore_richness: self.ore_richness,
            vegetation_density: self.vegetation_density,
            settlement_density: self.settlement_density,
            flat_world: false,
        };
        match self.world_type {
            WorldType::Continents => {}
            WorldType::Amplified => p.terrain_amplitude *= 1.9,
            WorldType::Islands => p.terrain_amplitude *= 0.85,
            WorldType::Flat => {
                p.flat_world = true;
                p.cave_density = 0.0;
            }
        }
        p
    }
}

/// プレイヤーの状態（セーブ対象）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerSave {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub hunger: f32,
    pub body_temp: f32,
    pub age_days: f32,
    pub money: f64,
    pub arrows: u32,
    pub selected_slot: usize,
    /// (アイテムキー, 個数) — キーで保存するのでプラグイン追加後もIDがずれない。
    pub hotbar: Vec<Option<(String, u32)>>,
    pub inventory: Vec<(String, u32)>,
    pub profession: String,
    pub reputation: f32,
    pub discovered_settlements: Vec<u64>,
}

/// 差分セーブの本体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSaveBody {
    pub format_version: u32,
    pub player: PlayerSave,
    /// 改変されたチャンクのみ。
    pub modified_chunks: Vec<PaletteRleChunk>,
    /// 世界史（因果イベントの要約）。
    pub chronicle: Vec<ChronicleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub tick: u64,
    pub year: u32,
    pub title: String,
    pub detail: String,
    pub importance: f32,
}

/// セーブ一覧に出す要約。
#[derive(Debug, Clone)]
pub struct SaveSlot {
    pub meta: WorldMeta,
    pub path: PathBuf,
    pub size_bytes: u64,
}

pub struct SaveManager {
    pub root: PathBuf,
}

impl SaveManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn saves_dir(&self) -> PathBuf {
        self.root.join("saves")
    }

    pub fn world_dir(&self, folder: &str) -> PathBuf {
        self.saves_dir().join(folder)
    }

    /// ワールド名をファイルシステムで安全な名前へ正規化する。
    pub fn sanitize_folder_name(name: &str) -> Result<String, SaveError> {
        let cleaned: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else if c.is_whitespace() {
                    '_'
                } else {
                    '-'
                }
            })
            .collect();
        let cleaned = cleaned.trim_matches(|c| c == '-' || c == '_').to_string();
        if cleaned.is_empty() {
            return Err(SaveError::BadName("空の名前は使えません".into()));
        }
        if cleaned.len() > 64 {
            return Ok(cleaned.chars().take(64).collect());
        }
        // Windows の予約デバイス名を避ける。
        const RESERVED: [&str; 8] = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "LPT1", "LPT2"];
        if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&cleaned)) {
            return Ok(format!("{cleaned}_world"));
        }
        Ok(cleaned)
    }

    /// 既存の名前と衝突しない一意なフォルダ名を返す。
    pub fn unique_folder_name(&self, display_name: &str) -> Result<String, SaveError> {
        let base = Self::sanitize_folder_name(display_name)?;
        if !self.world_dir(&base).exists() {
            return Ok(base);
        }
        for i in 2..1000 {
            let candidate = format!("{base}-{i}");
            if !self.world_dir(&candidate).exists() {
                return Ok(candidate);
            }
        }
        Err(SaveError::AlreadyExists(display_name.to_string()))
    }

    /// セーブ一覧。最終プレイ日時の新しい順。
    pub fn list_saves(&self) -> Vec<SaveSlot> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(self.saves_dir()) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let meta_path = path.join("world.json");
            let Ok(text) = std::fs::read_to_string(&meta_path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<WorldMeta>(&text) else {
                // 壊れたセーブは一覧から静かに除外する（起動を止めない）。
                continue;
            };
            let size = dir_size(&path);
            out.push(SaveSlot {
                meta,
                path,
                size_bytes: size,
            });
        }
        out.sort_by(|a, b| b.meta.last_played_unix.cmp(&a.meta.last_played_unix));
        out
    }

    /// 新しいワールドを作る（メタ情報のみ書き込む）。
    pub fn create_world(&self, mut meta: WorldMeta) -> Result<WorldMeta, SaveError> {
        let folder = self.unique_folder_name(&meta.display_name)?;
        meta.folder = folder.clone();
        meta.format_version = SAVE_FORMAT_VERSION;
        meta.created_unix = now_unix();
        meta.last_played_unix = meta.created_unix;

        let dir = self.world_dir(&folder);
        std::fs::create_dir_all(&dir)?;
        self.write_meta(&meta)?;

        // 空の本体を書き、読み込み可能な状態で作成完了とする。
        let body = WorldSaveBody {
            format_version: SAVE_FORMAT_VERSION,
            player: PlayerSave::default(),
            modified_chunks: Vec::new(),
            chronicle: Vec::new(),
        };
        self.write_body(&folder, &body)?;
        Ok(meta)
    }

    pub fn write_meta(&self, meta: &WorldMeta) -> Result<(), SaveError> {
        let dir = self.world_dir(&meta.folder);
        std::fs::create_dir_all(&dir)?;
        let text = serde_json::to_string_pretty(meta).map_err(|e| SaveError::Decode(e.to_string()))?;
        atomic_write(&dir.join("world.json"), text.as_bytes())?;
        Ok(())
    }

    pub fn read_meta(&self, folder: &str) -> Result<WorldMeta, SaveError> {
        let path = self.world_dir(folder).join("world.json");
        let text = std::fs::read_to_string(&path).map_err(|_| SaveError::NotFound(folder.to_string()))?;
        let meta: WorldMeta = serde_json::from_str(&text).map_err(|e| SaveError::Decode(e.to_string()))?;
        if meta.format_version != SAVE_FORMAT_VERSION {
            return Err(SaveError::Version {
                found: meta.format_version,
                expected: SAVE_FORMAT_VERSION,
            });
        }
        Ok(meta)
    }

    /// 世界本体を zstd 圧縮して書き出す。書き込み中の電断で壊れないよう、
    /// 一時ファイルへ書いてから置き換える。
    pub fn write_body(&self, folder: &str, body: &WorldSaveBody) -> Result<(), SaveError> {
        let dir = self.world_dir(folder);
        std::fs::create_dir_all(&dir)?;
        let raw = bincode::serialize(body).map_err(|e| SaveError::Decode(e.to_string()))?;

        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 6)?;
        encoder.write_all(&raw)?;
        let compressed = encoder.finish()?;

        atomic_write(&dir.join("world.dat"), &compressed)?;
        Ok(())
    }

    pub fn read_body(&self, folder: &str) -> Result<WorldSaveBody, SaveError> {
        let path = self.world_dir(folder).join("world.dat");
        let file = std::fs::File::open(&path).map_err(|_| SaveError::NotFound(folder.to_string()))?;
        let mut decoder = zstd::stream::Decoder::new(file)?;
        let mut raw = Vec::new();
        decoder.read_to_end(&mut raw)?;
        let body: WorldSaveBody =
            bincode::deserialize(&raw).map_err(|e| SaveError::Decode(e.to_string()))?;
        if body.format_version != SAVE_FORMAT_VERSION {
            return Err(SaveError::Version {
                found: body.format_version,
                expected: SAVE_FORMAT_VERSION,
            });
        }
        Ok(body)
    }

    pub fn delete_world(&self, folder: &str) -> Result<(), SaveError> {
        let dir = self.world_dir(folder);
        if !dir.exists() {
            return Err(SaveError::NotFound(folder.to_string()));
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// ワールドを複製する（バックアップ用）。
    pub fn duplicate_world(&self, folder: &str, new_display_name: &str) -> Result<WorldMeta, SaveError> {
        let mut meta = self.read_meta(folder)?;
        let body = self.read_body(folder)?;
        meta.display_name = new_display_name.to_string();
        let created = self.create_world(meta)?;
        self.write_body(&created.folder, &body)?;
        Ok(created)
    }
}

/// 改変チャンクの集合を保存形式へ変換する。
pub fn pack_modified_chunks(chunks: &HashMap<ChunkPos, ChunkData>) -> Vec<PaletteRleChunk> {
    chunks
        .values()
        .filter(|c| c.dirty_persist)
        .map(|c| c.to_palette_rle())
        .collect()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // Windows では既存ファイルがあると rename が失敗するため先に消す。
    let _ = std::fs::remove_file(path);
    std::fs::rename(&tmp, path)
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_size(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 既定値のメタ情報（ワールド作成画面の初期状態）。
pub fn default_meta(display_name: &str, seed: u64) -> WorldMeta {
    WorldMeta {
        format_version: SAVE_FORMAT_VERSION,
        display_name: display_name.to_string(),
        folder: String::new(),
        seed,
        world_type: WorldType::Continents,
        game_mode: GameMode::Survival,
        sea_level: crate::chunk::SEA_LEVEL,
        terrain_amplitude: 1.0,
        cave_density: 1.0,
        ore_richness: 1.0,
        vegetation_density: 1.0,
        settlement_density: 1.0,
        plugins: Vec::new(),
        played_seconds: 0.0,
        sim_tick: 0,
        created_unix: now_unix(),
        last_played_unix: now_unix(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{ids, BlockId};

    fn temp_root(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "wg_saves_{tag}_{}_{}",
            std::process::id(),
            now_unix()
        ));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn folder_names_are_sanitized() {
        assert_eq!(SaveManager::sanitize_folder_name("My World").unwrap(), "My_World");
        assert_eq!(SaveManager::sanitize_folder_name("../../etc/passwd").unwrap(), "etc-passwd");
        assert_eq!(SaveManager::sanitize_folder_name("C:\\evil").unwrap(), "C--evil");
        assert!(SaveManager::sanitize_folder_name("///").is_err());
        assert_eq!(SaveManager::sanitize_folder_name("CON").unwrap(), "CON_world");
        assert!(SaveManager::sanitize_folder_name(&"x".repeat(200)).unwrap().len() <= 64);
    }

    #[test]
    fn create_list_and_delete_worlds() {
        let root = temp_root("crud");
        let mgr = SaveManager::new(&root);
        assert!(mgr.list_saves().is_empty());

        let a = mgr.create_world(default_meta("最初の世界", 42)).unwrap();
        let b = mgr.create_world(default_meta("Second World", 99)).unwrap();
        assert_ne!(a.folder, b.folder);

        let saves = mgr.list_saves();
        assert_eq!(saves.len(), 2);
        assert!(saves.iter().all(|s| s.size_bytes > 0));

        mgr.delete_world(&a.folder).unwrap();
        assert_eq!(mgr.list_saves().len(), 1);
        assert!(matches!(mgr.delete_world(&a.folder), Err(SaveError::NotFound(_))));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_names_do_not_collide() {
        let root = temp_root("collide");
        let mgr = SaveManager::new(&root);
        let a = mgr.create_world(default_meta("World", 1)).unwrap();
        let b = mgr.create_world(default_meta("World", 2)).unwrap();
        let c = mgr.create_world(default_meta("World", 3)).unwrap();
        assert_eq!(a.folder, "World");
        assert_eq!(b.folder, "World-2");
        assert_eq!(c.folder, "World-3");
        assert_eq!(mgr.list_saves().len(), 3);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn body_round_trips_with_chunks_and_player() {
        let root = temp_root("body");
        let mgr = SaveManager::new(&root);
        let meta = mgr.create_world(default_meta("RoundTrip", 7)).unwrap();

        let mut chunk = ChunkData::empty(ChunkPos::new(-2, 5));
        for y in 0..40 {
            chunk.set(3, y, 3, ids::STONE);
        }
        chunk.set(3, 41, 3, ids::DIAMOND_ORE);
        chunk.dirty_persist = true;

        let mut map = HashMap::new();
        map.insert(chunk.pos, chunk);
        // 未改変チャンクは保存対象から外れること。
        let mut clean = ChunkData::empty(ChunkPos::new(9, 9));
        clean.set(0, 5, 0, ids::STONE);
        map.insert(clean.pos, clean);

        let body = WorldSaveBody {
            format_version: SAVE_FORMAT_VERSION,
            player: PlayerSave {
                x: 12.5,
                y: 70.0,
                z: -3.25,
                health: 87.0,
                hotbar: vec![Some(("genesis:iron_pickaxe".into(), 1)), None],
                profession: "鉱夫".into(),
                ..Default::default()
            },
            modified_chunks: pack_modified_chunks(&map),
            chronicle: vec![ChronicleEntry {
                tick: 1234,
                year: 3,
                title: "地震".into(),
                detail: "北の山脈で地震が発生した".into(),
                importance: 0.8,
            }],
        };
        assert_eq!(body.modified_chunks.len(), 1, "clean chunks must not be saved");

        mgr.write_body(&meta.folder, &body).unwrap();
        let loaded = mgr.read_body(&meta.folder).unwrap();

        assert_eq!(loaded.player.x, 12.5);
        assert_eq!(loaded.player.health, 87.0);
        assert_eq!(loaded.player.hotbar[0], Some(("genesis:iron_pickaxe".to_string(), 1)));
        assert_eq!(loaded.chronicle.len(), 1);
        assert_eq!(loaded.chronicle[0].title, "地震");
        assert_eq!(loaded.modified_chunks.len(), 1);

        let restored = ChunkData::from_palette_rle(&loaded.modified_chunks[0], &|b: BlockId| b.0 != 0).unwrap();
        assert_eq!(restored.pos, ChunkPos::new(-2, 5));
        assert_eq!(restored.get(3, 41, 3), ids::DIAMOND_ORE);
        assert_eq!(restored.get(3, 10, 3), ids::STONE);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn version_mismatch_is_reported_not_ignored() {
        let root = temp_root("version");
        let mgr = SaveManager::new(&root);
        let meta = mgr.create_world(default_meta("Old", 1)).unwrap();

        let mut stale = mgr.read_meta(&meta.folder).unwrap();
        stale.format_version = SAVE_FORMAT_VERSION + 99;
        let text = serde_json::to_string(&stale).unwrap();
        std::fs::write(mgr.world_dir(&meta.folder).join("world.json"), text).unwrap();

        match mgr.read_meta(&meta.folder) {
            Err(SaveError::Version { found, expected }) => {
                assert_eq!(found, SAVE_FORMAT_VERSION + 99);
                assert_eq!(expected, SAVE_FORMAT_VERSION);
            }
            other => panic!("expected a version error, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_save_is_skipped_in_the_listing() {
        let root = temp_root("corrupt");
        let mgr = SaveManager::new(&root);
        mgr.create_world(default_meta("Good", 1)).unwrap();
        let bad = mgr.saves_dir().join("Broken");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("world.json"), "not json at all").unwrap();

        let saves = mgr.list_saves();
        assert_eq!(saves.len(), 1, "the broken save must not break the listing");
        assert_eq!(saves[0].meta.display_name, "Good");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn world_types_map_to_distinct_generation_params() {
        let mut flat = default_meta("f", 1);
        flat.world_type = WorldType::Flat;
        assert!(flat.to_gen_params().flat_world);
        assert_eq!(flat.to_gen_params().cave_density, 0.0);

        let mut amp = default_meta("a", 1);
        amp.world_type = WorldType::Amplified;
        assert!(amp.to_gen_params().terrain_amplitude > 1.5);

        let normal = default_meta("n", 1);
        assert!(!normal.to_gen_params().flat_world);
        assert_eq!(normal.to_gen_params().terrain_amplitude, 1.0);
    }

    #[test]
    fn duplicating_a_world_copies_its_contents() {
        let root = temp_root("dup");
        let mgr = SaveManager::new(&root);
        let meta = mgr.create_world(default_meta("Original", 555)).unwrap();
        let body = WorldSaveBody {
            format_version: SAVE_FORMAT_VERSION,
            player: PlayerSave { x: 1.0, y: 2.0, z: 3.0, ..Default::default() },
            modified_chunks: Vec::new(),
            chronicle: Vec::new(),
        };
        mgr.write_body(&meta.folder, &body).unwrap();

        let copy = mgr.duplicate_world(&meta.folder, "Backup").unwrap();
        assert_ne!(copy.folder, meta.folder);
        assert_eq!(copy.seed, 555);
        assert_eq!(mgr.read_body(&copy.folder).unwrap().player.y, 2.0);
        assert_eq!(mgr.list_saves().len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
