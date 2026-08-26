---

## 4. `README.md`

```markdown
# World Genesis (完全自律型世界シミュレーションゲーム)

> **「開発者が世界の初期条件だけを設定し、開始ボタンを押した後は、世界そのものが時間の経過によって自律的に変化し続ける」**

---

## 主な機能

- **地質・テクトニクス**: プレート移動による造山運動・断層地震・水理侵食地形変化
- **気候・大気循環**: 太陽日射・大気3胞循環・降水凝結・季節変動
- **生態系循環**: 植物バイオマスと草食・肉食動物の個体群動態
- **社会・王朝・政治**: 家系図・長子相続・王の死による継承危機と内戦・国家間戦争
- **経済・為替**: 原材料から加工品への多段階生産連鎖・需給均衡市場・国家独自通貨Forex
- **歴史因果録 (Causality Ledger)**: 全事象の起因事象を過去に遡って完全追跡
- **3D描画 ＆ インタラクティブTUI**: リアルタイム地形観察・一市民としての実存プレイ

---

## ビルド ＆ 実行

### 前提条件

- Rust 1.75.0 以上

### テスト実行

```bash
cargo test
```

インタラクティブゲーム起動
code
Bash
cargo run --release -p genesis-sim
CLIツール（ヘッドレス生成・ベンチマーク）
code
Bash

# 500年シミュレーションと歴史年代記出力

cargo run --release -p genesis-tools -- simulate 500 64 world_chronicle.md

# 1,000年高負荷ベンチマーク

cargo run --release -p genesis-tools -- bench 1000 64
code
Code
---

## 5. 全体テスト ＆ 動作確認

```bash
# 1. 全クレート単体テスト & 長期不変量テスト
cargo test

# 2. CLIツールで200年シミュレーション＆年代記の生成
cargo run --release -p genesis-tools -- simulate 200 64 test_chronicle.md

# 3. インタラクティブ本編の起動
cargo run --release -p genesis-sim
```
