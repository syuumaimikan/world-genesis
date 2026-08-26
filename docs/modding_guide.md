# World Genesis: Mod制作ガイド (Modding Guide)

## 1. 概要

`assets/mods/*.json` に配置されたJSONファイルは、コアエンジンの再コンパイルなしで `ModRegistry` を通じてシミュレーションへロードされます。

## 2. アイテム定義スキーマ

```json
{
  "id": "unique_string_id",
  "display_name": "表示名",
  "base_value": 基準価格(f32),
  "weight_kg": 重量(f32),
  "is_perishable": 腐敗フラグ(bool)
}
```
