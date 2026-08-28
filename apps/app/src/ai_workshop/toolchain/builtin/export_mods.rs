// === AI-WORKSHOP START ===
// L2 工具链示例：将实例 mods 目录打包为 zip 导出。
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use async_zip::base::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use serde_json::json;

use super::super::toolchain_trait::ExecutableToolchain;
use crate::ai_workshop::other_err;
use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 打包统计：文件数与总字节数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackStats {
    pub file_count: usize,
    pub size: u64,
}

/// 递归收集 `dir` 下的普通文件，返回 `(绝对路径, 相对路径)` 列表；相对路径以 `prefix` 为前缀。
fn collect_files(
    dir: &Path,
    prefix: &Path,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = prefix.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(&path, &rel, out)?;
        } else if file_type.is_file() {
            out.push((path, rel));
        }
    }
    Ok(())
}

/// 纯函数：将 `src` 目录（含子目录递归）打包为 `dest_zip`。
/// 每打包 10 个文件（以及最后一个）调用 `progress(packed, percent, rel_path)`。
/// 返回 `PackStats`。此函数不依赖 theseus State，可独立测试。
pub async fn pack_mods_dir<F>(
    src: &Path,
    dest_zip: &Path,
    progress: F,
) -> Result<PackStats>
where
    F: FnMut(usize, f32, &Path) + Send,
{
    let mut files = Vec::new();
    collect_files(src, Path::new(""), &mut files)?;

    if let Some(parent) = dest_zip.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let file = tokio::fs::File::create(dest_zip).await?;
    let mut writer = ZipFileWriter::with_tokio(file);

    let total = files.len();
    let mut packed = 0usize;
    let mut size = 0u64;
    let mut progress = progress;
    for (path, rel) in &files {
        let bytes = tokio::fs::read(path).await?;
        size += bytes.len() as u64;
        let name = rel.to_string_lossy().replace('\\', "/");
        let opts = ZipEntryBuilder::new(name.into(), Compression::Deflate);
        writer
            .write_entry_whole(opts, &bytes)
            .await
            .map_err(|e| other_err(e.to_string()))?;
        packed += 1;
        if packed.is_multiple_of(10) || packed == total {
            let percent = if total == 0 {
                100.0
            } else {
                (packed as f32 / total as f32) * 100.0
            };
            progress(packed, percent, rel);
        }
    }
    writer.close().await.map_err(|e| other_err(e.to_string()))?;

    Ok(PackStats {
        file_count: packed,
        size,
    })
}

/// 将实例 mods 目录导出为 zip 备份的工具链。
pub struct ExportModsToolchain;

#[async_trait]
impl ExecutableToolchain for ExportModsToolchain {
    fn name(&self) -> &'static str {
        "export_mods"
    }

    fn description(&self) -> &'static str {
        "将实例 mods 目录打包为 zip 导出"
    }

    async fn execute(
        &self,
        instance_id: Option<&str>,
        _params: serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Result<serde_json::Value> {
        let instance_id =
            instance_id.ok_or_else(|| other_err("缺少 instance_id"))?;
        let root = theseus::instance::get_full_path(instance_id).await?;
        let mods_dir = root.join("mods");
        if !mods_dir.is_dir() {
            return Err(other_err(format!(
                "实例缺少 mods 目录: {}",
                mods_dir.display()
            )));
        }

        let export_dir = root.join("export");
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let dest_zip = export_dir.join(format!("mods-backup-{stamp}.zip"));

        let stats =
            pack_mods_dir(&mods_dir, &dest_zip, |packed, percent, path| {
                ctx.report_progress(
                    "packing",
                    Some(percent),
                    Some(path.to_string_lossy().to_string()),
                );
                let _ = packed;
            })
            .await?;

        Ok(json!({
            "path": dest_zip.to_string_lossy().to_string(),
            "file_count": stats.file_count,
            "size": stats.size,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建唯一临时目录（测试隔离，避免并发冲突）。
    fn temp_dir(tag: &str) -> PathBuf {
        let base = std::env::temp_dir()
            .join(format!("packrinth_{tag}_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    #[tokio::test]
    async fn packs_files_with_subdirectories_and_reports_progress() {
        let root = temp_dir("export_mods");
        let src = root.join("mods");
        let sub = src.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(src.join("a.jar"), b"aaa").unwrap();
        std::fs::write(src.join("b.disabled"), b"bbbb").unwrap();
        std::fs::write(sub.join("c.jar"), b"ccccc").unwrap();

        let dest = root.join("export").join("out.zip");
        let progress = std::sync::Mutex::new(Vec::new());
        let stats = pack_mods_dir(&src, &dest, |packed, percent, path| {
            progress.lock().unwrap().push((
                packed,
                percent,
                path.to_string_lossy().to_string(),
            ));
        })
        .await
        .unwrap();

        assert_eq!(stats.file_count, 3);
        assert_eq!(stats.size, 3 + 4 + 5);

        let events = progress.lock().unwrap();
        assert!(!events.is_empty());
        let last = events.last().unwrap();
        assert_eq!(last.0, 3);
        assert_eq!(last.1, 100.0);

        assert!(dest.exists());
    }

    #[tokio::test]
    async fn packs_nonexistent_src_is_error() {
        let root = temp_dir("export_mods_empty");
        let src = root.join("missing_mods");
        let dest = root.join("out.zip");
        let result = pack_mods_dir(&src, &dest, |_, _, _| {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn packs_empty_dir_succeeds_with_zero_files() {
        let root = temp_dir("export_mods_empty_dir");
        let src = root.join("mods");
        std::fs::create_dir_all(&src).unwrap();
        let dest = root.join("out.zip");
        let stats = pack_mods_dir(&src, &dest, |_, _, _| {}).await.unwrap();
        assert_eq!(stats.file_count, 0);
        assert!(dest.exists());
    }

    #[test]
    fn toolchain_metadata() {
        let toolchain = ExportModsToolchain;
        assert_eq!(toolchain.name(), "export_mods");
        assert!(!toolchain.description().is_empty());
    }
}
// === AI-WORKSHOP END ===
