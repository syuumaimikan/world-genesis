//! グリーディ・メッシング。
//!
//! 旧実装はブロック1個につき1エンティティ + 1つの立方体メッシュを生成していたため、
//! 描画半径4チャンクでも 2万を超えるエンティティと同数のドローコールが発生し、
//! フレームレートが破綻していた。
//!
//! ここでは 1チャンク = 最大3メッシュ（不透明・半透明・十字スプライト）へ統合し、
//! さらに隣接する同色の面を長方形へ結合する。平坦な地表 16×16 の上面は
//! 256枚のクアッドではなく 1枚のクアッドになるため、頂点数はおよそ 1/20〜1/100 になる。

use crate::blocks::{BlockId, BlockLookup, RenderClass};
use crate::lighting::{ao_factor, compute_lighting, corner_ao, LightSampler, LightVolume};
use crate::chunk::{ChunkData, CHUNK_H, CHUNK_SX, CHUNK_SZ};
use crate::noise::rand01_3i;
use std::sync::Arc;

/// 面の向き。頂点カラーの明度（擬似的な指向性ライティング）に使う。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    PosY,
    NegY,
    PosX,
    NegX,
    PosZ,
    NegZ,
}

impl Face {
    #[inline]
    fn normal(self) -> [f32; 3] {
        match self {
            Face::PosY => [0.0, 1.0, 0.0],
            Face::NegY => [0.0, -1.0, 0.0],
            Face::PosX => [1.0, 0.0, 0.0],
            Face::NegX => [-1.0, 0.0, 0.0],
            Face::PosZ => [0.0, 0.0, 1.0],
            Face::NegZ => [0.0, 0.0, -1.0],
        }
    }

    /// 方向ごとの陰影。上面が最も明るく、底面が最も暗い。
    #[inline]
    fn shade(self) -> f32 {
        match self {
            Face::PosY => 1.0,
            Face::NegY => 0.52,
            Face::PosX | Face::NegX => 0.76,
            Face::PosZ | Face::NegZ => 0.88,
        }
    }
}

/// 生成されたメッシュのCPU側バッファ。Bevy の `Mesh` へは main スレッドで変換する。
#[derive(Default, Clone)]
pub struct MeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshBuffers {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }

    /// 4頂点それぞれに別の色を持たせる（環境遮蔽の陰影に使う）。
    fn push_quad_shaded(&mut self, verts: [[f32; 3]; 4], normal: [f32; 3], colors: [[f32; 4]; 4]) {
        let base = self.positions.len() as u32;
        for (v, c) in verts.into_iter().zip(colors) {
            self.positions.push(v);
            self.normals.push(normal);
            self.colors.push(c);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn push_quad(&mut self, verts: [[f32; 3]; 4], normal: [f32; 3], color: [f32; 4]) {
        let base = self.positions.len() as u32;
        for v in verts {
            self.positions.push(v);
            self.normals.push(normal);
            self.colors.push(color);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// 1チャンク分の描画データ。
#[derive(Default, Clone)]
pub struct ChunkMeshes {
    pub opaque: MeshBuffers,
    pub translucent: MeshBuffers,
    pub cross: MeshBuffers,
}

/// メッシュ生成の入力。ワーカースレッドへ move するため所有データだけを持つ。
/// チャンク本体は `Arc` で共有し、80KB のボクセル配列を複製しないようにしてある。
pub struct MeshInput {
    pub center: Arc<ChunkData>,
    /// -X, +X, -Z, +Z の隣接チャンク。境界面のカリングに必要。
    pub neighbors: [Option<Arc<ChunkData>>; 4],
    pub lookup: BlockLookup,
    pub seed: u64,
}

impl MeshInput {
    /// チャンクローカル座標を越えた参照を、隣接チャンクへ委譲して解決する。
    #[inline]
    fn block(&self, lx: i32, y: i32, lz: i32) -> BlockId {
        if !(0..CHUNK_H).contains(&y) {
            return BlockId(0);
        }
        if lx < 0 {
            return match &self.neighbors[0] {
                Some(c) => c.get(lx + CHUNK_SX, y, lz.clamp(0, CHUNK_SZ - 1)),
                // 未生成の隣接チャンクは「不透明な壁」とみなす。穴が見えるより自然で、
                // 隣が届いた時点で再メッシュされる。
                None => BlockId(u16::MAX),
            };
        }
        if lx >= CHUNK_SX {
            return match &self.neighbors[1] {
                Some(c) => c.get(lx - CHUNK_SX, y, lz.clamp(0, CHUNK_SZ - 1)),
                None => BlockId(u16::MAX),
            };
        }
        if lz < 0 {
            return match &self.neighbors[2] {
                Some(c) => c.get(lx, y, lz + CHUNK_SZ),
                None => BlockId(u16::MAX),
            };
        }
        if lz >= CHUNK_SZ {
            return match &self.neighbors[3] {
                Some(c) => c.get(lx, y, lz - CHUNK_SZ),
                None => BlockId(u16::MAX),
            };
        }
        self.center.get(lx, y, lz)
    }

    #[inline]
    fn is_opaque(&self, id: BlockId) -> bool {
        // 未生成チャンクを表す番兵は常に不透明扱い。
        id.0 == u16::MAX || self.lookup.is_opaque(id)
    }
}

/// ある面を描画すべきかどうか。
#[inline]
fn face_visible(input: &MeshInput, here: BlockId, there: BlockId) -> bool {
    if there.0 == u16::MAX {
        return false; // 未生成の隣へは面を張らない
    }
    if there.is_air() {
        return true;
    }
    if input.is_opaque(there) {
        return false;
    }
    // 半透明同士（水と水など）は内部の面を張らない。
    here != there
}

/// 光の計算に使う、中心チャンク周辺の余白（ブロック）。
/// 松明の光（最大 15）が隣チャンクから差し込む分を拾うための幅。
pub const LIGHT_PAD: i32 = 8;

/// `MeshInput` を光量計算から読めるようにする薄いアダプタ。
struct MeshLightSampler<'a> {
    input: &'a MeshInput,
}

impl LightSampler for MeshLightSampler<'_> {
    #[inline]
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        // グリッド座標をチャンクローカルへ戻す。
        let b = self.input.block(x - LIGHT_PAD, y, z - LIGHT_PAD);
        // 未生成の隣を表す番兵は、光計算では不透明な岩として扱う。
        if b.0 == u16::MAX {
            crate::blocks::ids::STONE
        } else {
            b
        }
    }
}

/// チャンク1つ分のメッシュを構築する。純関数なのでワーカースレッドで安全に実行できる。
pub fn build_chunk_meshes(input: &MeshInput) -> ChunkMeshes {
    // 面ごとの明るさと環境遮蔽をここで一度だけ求め、頂点カラーへ焼き込む。
    let sampler = MeshLightSampler { input };
    let light = compute_lighting(
        &sampler,
        &input.lookup,
        CHUNK_SX + LIGHT_PAD * 2,
        CHUNK_H,
        CHUNK_SZ + LIGHT_PAD * 2,
        LIGHT_PAD,
    );
    let ctx = MeshContext { input, light };
    build_chunk_meshes_with(&ctx)
}

/// 光量を含む描画コンテキスト。
struct MeshContext<'a> {
    input: &'a MeshInput,
    light: LightVolume,
}

impl MeshContext<'_> {
    /// チャンクローカル座標の明るさ（0.0〜1.0）。
    ///
    /// 焼き込みは常に「真昼」の強さで行う。昼夜の暗さは太陽光と環境光が
    /// 場面全体に掛けてくれるので、ここで時刻を織り込むと、時間が進むたびに
    /// 全チャンクを作り直す羽目になる。焼き込むのは「洞窟は空が見えないから
    /// 暗い」「松明の周りは明るい」という**場所による**明暗だけでよい。
    #[inline]
    fn light_at(&self, lx: i32, y: i32, lz: i32) -> f32 {
        self.light.illumination(lx, y, lz, 1.0)
    }

    /// この座標が AO の遮蔽物になるか。
    #[inline]
    fn occludes(&self, lx: i32, y: i32, lz: i32) -> bool {
        let b = self.input.block(lx, y, lz);
        b.0 == u16::MAX || self.input.lookup.is_opaque(b)
    }
}

fn build_chunk_meshes_with(ctx: &MeshContext) -> ChunkMeshes {
    let input = ctx.input;
    let mut out = ChunkMeshes::default();

    greedy_axis(ctx, &mut out, Face::PosY);
    greedy_axis(ctx, &mut out, Face::NegY);
    greedy_axis(ctx, &mut out, Face::PosX);
    greedy_axis(ctx, &mut out, Face::NegX);
    greedy_axis(ctx, &mut out, Face::PosZ);
    greedy_axis(ctx, &mut out, Face::NegZ);

    build_cross_sprites(ctx, &mut out);
    let _ = input;
    out
}

/// 面の融合キー。同じキーの隣接面だけが1枚の長方形へまとめられる。
///
/// 明るさと環境遮蔽もキーに含める。これを入れないと、暗い面と明るい面が
/// 1枚に融合してしまい、洞窟の陰も角の陰影も消えてしまう。
#[derive(Clone, Copy, PartialEq)]
struct FaceKey {
    block: u16,
    /// 量子化した色（浮動小数の誤差でマージが崩れないようにする）。
    tint: u16,
    /// 量子化した面の明るさ。
    light: u8,
    /// 4隅の環境遮蔽（0〜3）。
    ao: [u8; 4],
    translucent: bool,
}

const NO_FACE: FaceKey = FaceKey {
    block: 0,
    tint: 0,
    light: 0,
    ao: [3; 4],
    translucent: false,
};

impl FaceKey {
    #[inline]
    fn is_none(self) -> bool {
        self.block == 0
    }
}

/// 1軸分のグリーディ・メッシング。
fn greedy_axis(ctx: &MeshContext, out: &mut ChunkMeshes, face: Face) {
    let input = ctx.input;
    // 各軸を (u, v, slice) の3次元へ写像する。
    // slice が走査する層、u/v がその層内の2次元。
    let (su, sv, ss): (i32, i32, i32) = match face {
        Face::PosY | Face::NegY => (CHUNK_SX, CHUNK_SZ, CHUNK_H),
        Face::PosX | Face::NegX => (CHUNK_SZ, CHUNK_H, CHUNK_SX),
        Face::PosZ | Face::NegZ => (CHUNK_SX, CHUNK_H, CHUNK_SZ),
    };

    // 層内の面マスク。
    let mut mask: Vec<FaceKey> = vec![NO_FACE; (su * sv) as usize];
    let mut merged: Vec<bool> = vec![false; (su * sv) as usize];

    let to_xyz = |u: i32, v: i32, s: i32| -> (i32, i32, i32) {
        match face {
            Face::PosY | Face::NegY => (u, s, v),
            Face::PosX | Face::NegX => (s, v, u),
            Face::PosZ | Face::NegZ => (u, v, s),
        }
    };
    let n = face.normal();
    let (nx, ny, nz) = (n[0] as i32, n[1] as i32, n[2] as i32);

    for s in 0..ss {
        // --- マスクを作る ---
        let mut any = false;
        for v in 0..sv {
            for u in 0..su {
                let (x, y, z) = to_xyz(u, v, s);
                let here = input.center.get(x, y, z);
                let idx = (v * su + u) as usize;
                mask[idx] = NO_FACE;
                merged[idx] = false;

                if here.is_air() {
                    continue;
                }
                let entry = input.lookup.entry(here);
                if matches!(entry.render, RenderClass::Cross) {
                    continue; // 十字スプライトは別処理
                }
                let there = input.block(x + nx, y + ny, z + nz);
                if !face_visible(input, here, there) {
                    continue;
                }

                // 面ごとの明度をブロック座標のハッシュで微妙にばらつかせ、
                // 単色の平面がのっぺりして見えるのを防ぐ（疑似テクスチャ）。
                let jitter = (rand01_3i(x, y, z, input.seed ^ 0x7A17) - 0.5) * entry.grain;
                let tint = quantize_tint(face.shade() * (1.0 + jitter));

                // 明るさは「面の手前の空きマス」で測る。ブロックの中ではなく、
                // 光が当たっている側の値を使うのが正しい。
                let (ax, ay, az) = (x + nx, y + ny, z + nz);
                let light = quantize_light(ctx.light_at(ax, ay, az));
                let ao = face_ao(ctx, face, ax, ay, az);

                mask[idx] = FaceKey {
                    block: here.0,
                    tint,
                    light,
                    ao,
                    translucent: matches!(entry.render, RenderClass::Translucent),
                };
                any = true;
            }
        }
        if !any {
            continue;
        }

        // --- マスクを長方形へ融合する ---
        for v in 0..sv {
            let mut u = 0;
            while u < su {
                let idx = (v * su + u) as usize;
                let key = mask[idx];
                if key.is_none() || merged[idx] {
                    u += 1;
                    continue;
                }

                // 横方向へ伸ばす。
                let mut w = 1;
                while u + w < su {
                    let i = (v * su + u + w) as usize;
                    if merged[i] || mask[i] != key {
                        break;
                    }
                    w += 1;
                }

                // 縦方向へ伸ばす（行全体が一致するときだけ）。
                let mut h = 1;
                'grow: while v + h < sv {
                    for k in 0..w {
                        let i = ((v + h) * su + u + k) as usize;
                        if merged[i] || mask[i] != key {
                            break 'grow;
                        }
                    }
                    h += 1;
                }

                for dv in 0..h {
                    for du in 0..w {
                        merged[((v + dv) * su + u + du) as usize] = true;
                    }
                }

                emit_quad(out, ctx, face, to_xyz, u, v, s, w, h, key);
                u += w;
            }
        }
    }
}

#[inline]
fn quantize_tint(t: f32) -> u16 {
    (t.clamp(0.0, 2.0) * 2048.0) as u16
}

#[inline]
fn dequantize_tint(t: u16) -> f32 {
    t as f32 / 2048.0
}

#[allow(clippy::too_many_arguments)]
fn emit_quad(
    out: &mut ChunkMeshes,
    ctx: &MeshContext,
    face: Face,
    to_xyz: impl Fn(i32, i32, i32) -> (i32, i32, i32),
    u: i32,
    v: i32,
    s: i32,
    w: i32,
    h: i32,
    key: FaceKey,
) {
    let entry = ctx.input.lookup.entry(BlockId(key.block));
    let base = match face {
        Face::PosY => entry.color_top,
        Face::NegY => entry.color_bottom,
        _ => entry.color_side,
    };
    let t = dequantize_tint(key.tint);
    // 焼き込んだ明るさ。真っ黒だと何も見えないので下限を残す。
    let lit = 0.06 + 0.94 * dequantize_light(key.light);
    let alpha = if key.translucent {
        if entry.liquid {
            // はっきり水底が透けるくらいに薄くする。
            0.55
        } else {
            0.45
        }
    } else {
        1.0
    };
    let color = [
        (base[0] * t * lit).clamp(0.0, 1.0),
        (base[1] * t * lit).clamp(0.0, 1.0),
        (base[2] * t * lit).clamp(0.0, 1.0),
        alpha,
    ];

    // 面の4隅を (u,v,slice) 空間で作り、ワールド空間へ写す。
    // 面は s または s+1 の平面上にある。
    let offset = match face {
        Face::PosY | Face::PosX | Face::PosZ => 1,
        _ => 0,
    };
    let corners_uv = [(u, v), (u + w, v), (u + w, v + h), (u, v + h)];
    let mut verts = [[0.0f32; 3]; 4];
    for (i, (cu, cv)) in corners_uv.iter().enumerate() {
        let (x, y, z) = to_xyz(*cu, *cv, s + offset);
        verts[i] = [x as f32, y as f32, z as f32];
    }

    // (u,v) の並びをそのまま使うと、軸によっては三角形の巻き方向が法線と逆になる。
    // 外積が法線と同じ向きを指すよう、該当する面だけ頂点順を反転する。
    // 隅ごとの環境遮蔽を頂点カラーへ乗せる。頂点の並び替えに追従させる。
    let mut ao = key.ao;
    if matches!(face, Face::PosY | Face::PosX | Face::NegZ) {
        verts.swap(1, 3);
        ao.swap(1, 3);
    }

    let mut colors = [color; 4];
    for i in 0..4 {
        let f = ao_factor(ao[i]);
        colors[i][0] *= f;
        colors[i][1] *= f;
        colors[i][2] *= f;
    }

    let buf = if key.translucent {
        &mut out.translucent
    } else {
        &mut out.opaque
    };
    buf.push_quad_shaded(verts, face.normal(), colors);
}

/// 面の 4 隅における環境遮蔽を求める。
///
/// `(ax, ay, az)` は面の手前の空きマス。その周囲 8 近傍のうち、
/// 隅ごとに関わる 3 マス（辺2つと対角1つ）が埋まっているかを見る。
fn face_ao(ctx: &MeshContext, face: Face, ax: i32, ay: i32, az: i32) -> [u8; 4] {
    // 面の平面内で直交する 2 本の軸。
    let (ua, va) = match face {
        Face::PosY | Face::NegY => ((1, 0, 0), (0, 0, 1)),
        Face::PosX | Face::NegX => ((0, 0, 1), (0, 1, 0)),
        Face::PosZ | Face::NegZ => ((1, 0, 0), (0, 1, 0)),
    };
    // corners_uv と同じ並び: (0,0) (1,0) (1,1) (0,1)
    const SIGNS: [(i32, i32); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
    let mut out = [3u8; 4];
    for (i, (su, sv)) in SIGNS.iter().enumerate() {
        let s1 = ctx.occludes(ax + ua.0 * su, ay + ua.1 * su, az + ua.2 * su);
        let s2 = ctx.occludes(ax + va.0 * sv, ay + va.1 * sv, az + va.2 * sv);
        let c = ctx.occludes(
            ax + ua.0 * su + va.0 * sv,
            ay + ua.1 * su + va.1 * sv,
            az + ua.2 * su + va.2 * sv,
        );
        out[i] = corner_ao(s1, s2, c);
    }
    out
}

#[inline]
fn quantize_light(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0) as u8
}

#[inline]
fn dequantize_light(v: u8) -> f32 {
    v as f32 / 255.0
}

/// 草・花などの十字スプライトを生成する。
fn build_cross_sprites(ctx: &MeshContext, out: &mut ChunkMeshes) {
    let input = ctx.input;
    for lz in 0..CHUNK_SZ {
        for lx in 0..CHUNK_SX {
            for y in 0..CHUNK_H {
                let id = input.center.get(lx, y, lz);
                if id.is_air() {
                    continue;
                }
                let entry = input.lookup.entry(id);
                if !matches!(entry.render, RenderClass::Cross) {
                    continue;
                }

                let jitter = (rand01_3i(lx, y, lz, input.seed ^ 0x3C05) - 0.5) * entry.grain;
                let lit = 0.06 + 0.94 * ctx.light_at(lx, y, lz);
                let c = entry.color_top;
                let color = [
                    (c[0] * (1.0 + jitter) * lit).clamp(0.0, 1.0),
                    (c[1] * (1.0 + jitter) * lit).clamp(0.0, 1.0),
                    (c[2] * (1.0 + jitter) * lit).clamp(0.0, 1.0),
                    1.0,
                ];

                // 同じ位置でも微妙に向きと大きさを変え、規則性を消す。
                let ox = (rand01_3i(lx, y, lz, input.seed ^ 0x11) - 0.5) * 0.3;
                let oz = (rand01_3i(lx, y, lz, input.seed ^ 0x22) - 0.5) * 0.3;
                let hgt = 0.72 + rand01_3i(lx, y, lz, input.seed ^ 0x33) * 0.5;

                let (fx, fy, fz) = (lx as f32 + 0.5 + ox, y as f32, lz as f32 + 0.5 + oz);
                let r = 0.45;

                // 交差する2枚の板。裏面も描くため両方向のクアッドを積む。
                for (dx, dz) in [(r, r), (r, -r)] {
                    let quad = [
                        [fx - dx, fy, fz - dz],
                        [fx + dx, fy, fz + dz],
                        [fx + dx, fy + hgt, fz + dz],
                        [fx - dx, fy + hgt, fz - dz],
                    ];
                    let n = [-dz, 0.0, dx];
                    let inv = [dz, 0.0, -dx];
                    out.cross.push_quad(quad, n, color);
                    let mut back = quad;
                    back.swap(1, 3);
                    out.cross.push_quad(back, inv, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{ids, BlockRegistry};
    use crate::chunk::ChunkPos;

    fn input_from(chunk: ChunkData) -> MeshInput {
        MeshInput {
            center: Arc::new(chunk),
            neighbors: [None, None, None, None],
            lookup: BlockRegistry::with_builtins().snapshot(),
            seed: 42,
        }
    }

    fn full_neighbors(chunk: ChunkData) -> MeshInput {
        // 全周を空気チャンクで囲む（外周の面が描かれることを保証する）。
        let air = || Some(Arc::new(ChunkData::empty(ChunkPos::new(0, 0))));
        MeshInput {
            center: Arc::new(chunk),
            neighbors: [air(), air(), air(), air()],
            lookup: BlockRegistry::with_builtins().snapshot(),
            seed: 42,
        }
    }

    #[test]
    fn empty_chunk_produces_no_geometry() {
        let m = build_chunk_meshes(&input_from(ChunkData::empty(ChunkPos::new(0, 0))));
        assert!(m.opaque.is_empty() && m.translucent.is_empty() && m.cross.is_empty());
    }

    #[test]
    fn single_block_has_exactly_six_quads() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        c.set(8, 40, 8, ids::STONE);
        let m = build_chunk_meshes(&input_from(c));
        assert_eq!(m.opaque.quad_count(), 6, "an isolated cube must have 6 faces");
        assert_eq!(m.opaque.positions.len(), 24);
        assert_eq!(m.opaque.indices.len(), 36);
    }

    #[test]
    fn greedy_merging_collapses_a_flat_slab() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                c.set(lx, 40, lz, ids::STONE);
            }
        }
        // grain によるばらつきを消すため、色ジッタの無い状態で比較する。
        let mut input = full_neighbors(c);
        input.lookup.entries[ids::STONE.0 as usize].grain = 0.0;
        let m = build_chunk_meshes(&input);

        // 環境遮蔽が入ると、陰の違う面同士は融合できない（縁と内側で影が違う）。
        // それでも素朴な実装の 16*16*2 + 16*4 = 576 枚からは桁で減るはず。
        let quads = m.opaque.quad_count();
        assert!(quads < 60, "greedy meshing barely merged the slab: {quads} quads");
        // 実際に「1枚で複数ブロックを覆うクアッド」が存在すること。
        let widest = (0..m.opaque.quad_count())
            .map(|q| {
                let vs = &m.opaque.positions[q * 4..q * 4 + 4];
                let dx = vs.iter().map(|v| v[0]).fold(f32::MIN, f32::max)
                    - vs.iter().map(|v| v[0]).fold(f32::MAX, f32::min);
                let dz = vs.iter().map(|v| v[2]).fold(f32::MIN, f32::max)
                    - vs.iter().map(|v| v[2]).fold(f32::MAX, f32::min);
                dx.max(dz)
            })
            .fold(0.0f32, f32::max);
        assert!(widest > 1.5, "no quad spans more than a single block (widest={widest})");
    }

    #[test]
    fn interior_faces_are_culled() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        for y in 30..40 {
            for lz in 0..CHUNK_SZ {
                for lx in 0..CHUNK_SX {
                    c.set(lx, y, lz, ids::STONE);
                }
            }
        }
        let mut input = full_neighbors(c);
        input.lookup.entries[ids::STONE.0 as usize].grain = 0.0;
        let m = build_chunk_meshes(&input);
        // 直方体の内部に面が残っていないこと（残ると中身が見えてしまう）。
        // 立体の内側 y=31..39 の平面上に面があってはならない。
        for q in 0..m.opaque.quad_count() {
            let vs = &m.opaque.positions[q * 4..q * 4 + 4];
            let ys: Vec<f32> = vs.iter().map(|v| v[1]).collect();
            let flat_y = ys.iter().all(|y| (*y - ys[0]).abs() < 1e-4);
            if flat_y {
                let y = ys[0];
                assert!(
                    !(30.5..39.5).contains(&y),
                    "an interior horizontal face survived at y={y}"
                );
            }
        }
    }

    #[test]
    fn missing_neighbor_does_not_create_a_wall_of_faces() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        for y in 30..40 {
            for lz in 0..CHUNK_SZ {
                for lx in 0..CHUNK_SX {
                    c.set(lx, y, lz, ids::STONE);
                }
            }
        }
        // 隣接チャンク無し = 境界面は描かない（隣が届いた時点で貼り直す）。
        let mut input = input_from(c);
        input.lookup.entries[ids::STONE.0 as usize].grain = 0.0;
        let m = build_chunk_meshes(&input);
        for q in 0..m.opaque.quad_count() {
            let vs = &m.opaque.positions[q * 4..q * 4 + 4];
            for axis in [0usize, 2usize] {
                let vals: Vec<f32> = vs.iter().map(|v| v[axis]).collect();
                let flat = vals.iter().all(|v| (*v - vals[0]).abs() < 1e-4);
                if flat {
                    let v = vals[0];
                    assert!(
                        v > 0.001 && v < 15.999,
                        "a face was emitted on the un-generated chunk boundary (axis {axis}, v={v})"
                    );
                }
            }
        }
    }

    #[test]
    fn water_and_stone_go_to_separate_buffers() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        c.set(4, 20, 4, ids::STONE);
        c.set(6, 20, 6, ids::WATER);
        let m = build_chunk_meshes(&input_from(c));
        assert_eq!(m.opaque.quad_count(), 6);
        assert_eq!(m.translucent.quad_count(), 6);
        assert!(m.translucent.colors.iter().all(|c| c[3] < 1.0), "water must be translucent");
        assert!(m.opaque.colors.iter().all(|c| c[3] == 1.0));
    }

    #[test]
    fn adjacent_water_does_not_generate_internal_faces() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        for lz in 0..4 {
            for lx in 0..4 {
                for y in 20..24 {
                    c.set(lx, y, lz, ids::WATER);
                }
            }
        }
        let mut input = full_neighbors(c);
        input.lookup.entries[ids::WATER.0 as usize].grain = 0.0;
        let m = build_chunk_meshes(&input);
        // 水塊の内部に面が残っていないこと（残ると水中が縞模様になる）。
        assert!(m.translucent.quad_count() > 0, "water produced no surface at all");
        for q in 0..m.translucent.quad_count() {
            let vs = &m.translucent.positions[q * 4..q * 4 + 4];
            let ys: Vec<f32> = vs.iter().map(|v| v[1]).collect();
            if ys.iter().all(|y| (*y - ys[0]).abs() < 1e-4) {
                let y = ys[0];
                assert!(
                    !(20.5..23.5).contains(&y),
                    "an internal water face survived at y={y}"
                );
            }
        }
    }

    #[test]
    fn cross_blocks_produce_sprite_geometry_only() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        c.set(2, 30, 2, ids::TALL_GRASS);
        let m = build_chunk_meshes(&input_from(c));
        assert!(m.opaque.is_empty(), "cross blocks must not emit cube faces");
        // 2枚の板 × 表裏 = 4クアッド。
        assert_eq!(m.cross.quad_count(), 4);
    }

    #[test]
    fn mesh_buffers_are_internally_consistent() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                for y in 0..30 {
                    c.set(lx, y, lz, ids::DIRT);
                }
                c.set(lx, 30, lz, ids::GRASS);
                c.set(lx, 31, lz, ids::TALL_GRASS);
            }
        }
        let m = build_chunk_meshes(&full_neighbors(c));
        for buf in [&m.opaque, &m.translucent, &m.cross] {
            assert_eq!(buf.positions.len(), buf.normals.len());
            assert_eq!(buf.positions.len(), buf.colors.len());
            assert_eq!(buf.indices.len() % 6, 0);
            for &i in &buf.indices {
                assert!((i as usize) < buf.positions.len(), "index out of range");
            }
            for p in &buf.positions {
                assert!(p.iter().all(|v| v.is_finite()));
            }
            for c in &buf.colors {
                assert!(c.iter().all(|v| (0.0..=1.0).contains(v)), "colour out of range: {c:?}");
            }
        }
    }

    #[test]
    fn quads_wind_consistently_with_their_normals() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        c.set(8, 40, 8, ids::STONE);
        let m = build_chunk_meshes(&input_from(c));
        // 各三角形の外積が、頂点に記録された法線と同じ向きを指すこと。
        for tri in m.opaque.indices.chunks(3) {
            let p0 = m.opaque.positions[tri[0] as usize];
            let p1 = m.opaque.positions[tri[1] as usize];
            let p2 = m.opaque.positions[tri[2] as usize];
            let n = m.opaque.normals[tri[0] as usize];
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let dot = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2];
            assert!(dot > 0.0, "triangle winding disagrees with its normal (dot={dot})");
        }
    }
}
