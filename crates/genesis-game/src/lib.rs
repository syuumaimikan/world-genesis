//! World Genesis — ボクセル3Dクライアントの中核ライブラリ。
//!
//! Bevy に依存しない純粋なシミュレーション部（ワールド生成・メッシング・
//! セーブ・プラグイン）と、Bevy 上で動く描画・UI 部を同じクレートに置き、
//! 前者を単体テスト可能な純関数として保つ構成にしている。

pub mod actors;
pub mod ai;
pub mod anatomy;
pub mod astronomy;
pub mod attributes;
pub mod biome;
pub mod blocks;
pub mod blocky;
pub mod chronicle;
pub mod chunk;
pub mod dev;
pub mod disease;
pub mod fluid;
pub mod fluid_sim;
pub mod game;
pub mod hud;
pub mod items;
pub mod keybinds;
pub mod lighting;
pub mod magic;
pub mod menu;
pub mod mesher;
pub mod noise;
pub mod physics;
pub mod plugins;
pub mod saves;
pub mod settings;
pub mod species;
pub mod streaming;
pub mod ui_theme;
pub mod village;
pub mod worldgen;
