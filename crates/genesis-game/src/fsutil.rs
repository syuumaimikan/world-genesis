//! ディスクへの書き込みを1か所へまとめた小さなユーティリティ。
//!
//! セーブデータ・設定・プラグインの雛形はどれも「親ディレクトリを作り、
//! 一時ファイルへ書いてから置き換える」という同じ手順を必要とする。
//! 途中で電源が落ちても既存のファイルが半端な内容にならないことが重要なので、
//! 個々の呼び出し側で書き分けるのではなく、ここだけを通す。

use std::io::Write;
use std::path::Path;

/// 親ディレクトリを（無ければ）作る。
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// 一時ファイル経由で置き換える書き込み。
///
/// 書き込み途中のファイルが本来の名前で観測されることはない。
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    ensure_parent_dir(path)?;
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

/// 値を整形済み JSON として `atomic_write` する。
pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    atomic_write(path, text.as_bytes())
}

/// ディレクトリ以下のファイルサイズの合計（サブディレクトリも辿る）。
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    #[test]
    fn atomic_write_creates_missing_parents_and_leaves_no_temp_file() {
        let root = temp_dir("fsutil_parents");
        let path = root.join("deep").join("nested").join("data.bin");
        atomic_write(&path, b"hello").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        assert!(
            !path.with_extension("tmp").exists(),
            "temp file was left behind"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_write_replaces_existing_content_entirely() {
        let root = temp_dir("fsutil_replace");
        let path = root.join("data.bin");
        atomic_write(&path, b"a longer previous content").unwrap();
        atomic_write(&path, b"short").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"short");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dir_size_counts_nested_files() {
        let root = temp_dir("fsutil_size");
        atomic_write(&root.join("a.bin"), &[0u8; 100]).unwrap();
        atomic_write(&root.join("sub").join("b.bin"), &[0u8; 23]).unwrap();

        assert_eq!(dir_size(&root), 123);
        assert_eq!(
            dir_size(&root.join("missing")),
            0,
            "a missing dir must not panic"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
