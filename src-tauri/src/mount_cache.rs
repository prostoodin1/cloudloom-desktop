//! Remove only rclone files positively identified as clean. Never recurse-delete a buffer.
use super::*;
fn regular(path: &Path, directory: bool) -> Result<bool, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Буфер недоступен для очистки".into()),
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Ссылки и точки подключения в буфере не очищаются".into());
        }
    }
    if metadata.file_type().is_symlink()
        || metadata.is_dir() != directory
        || (!directory && !metadata.is_file())
    {
        return Err("Неизвестный элемент в буфере; автоматическая очистка остановлена".into());
    }
    Ok(true)
}
fn walk(root: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    if !regular(root, true)? {
        return Ok((vec![], vec![]));
    }
    let mut dirs = vec![root.to_path_buf()];
    let mut files = vec![];
    let mut index = 0;
    while index < dirs.len() {
        let parent = dirs[index].clone();
        index += 1;
        for row in std::fs::read_dir(parent).map_err(|_| "Не удалось прочитать буфер")?
        {
            let row = row.map_err(|_| "Не удалось прочитать буфер")?;
            let path = row.path();
            if row
                .file_type()
                .map_err(|_| "Не удалось проверить буфер")?
                .is_dir()
            {
                regular(&path, true)?;
                dirs.push(path);
            } else {
                regular(&path, false)?;
                files.push(path);
            }
            if files.len() + dirs.len() > 100_000 {
                return Err("Буфер слишком велик для одной безопасной очистки".into());
            }
        }
    }
    Ok((dirs, files))
}
pub(super) fn clean(cache: &Path) -> Result<u64, String> {
    if !regular(cache, true)? {
        return Ok(0);
    }
    let meta = cache.join("vfsMeta");
    let data = cache.join("vfs");
    let (_, files) = walk(&meta)?;
    let (data_dirs, _) = walk(&data)?;
    let mut confirmed = vec![];
    for file in files {
        if std::fs::metadata(&file)
            .map_err(|_| "Метаданные буфера недоступны")?
            .len()
            > 2 * 1024 * 1024
        {
            return Err("Слишком большие метаданные буфера".into());
        }
        let value: Value = serde_json::from_slice(
            &std::fs::read(&file).map_err(|_| "Не удалось прочитать метаданные буфера")?,
        )
        .map_err(|_| "Метаданные буфера не распознаны; файлы сохранены")?;
        if value["Dirty"].as_bool() != Some(false) {
            return Err("Неотправленные или неизвестные файлы буфера сохранены".into());
        }
        let relative = file
            .strip_prefix(&meta)
            .map_err(|_| "Неверный путь буфера")?;
        let local = data.join(relative);
        regular(&local, false)?;
        confirmed.push((local, file));
    }
    let mut removed = 0;
    for (local, metadata) in confirmed {
        match std::fs::remove_file(&local) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => continue, // Still in use: preserve both data and its metadata.
        }
        let _ = std::fs::remove_file(metadata);
    }
    let (meta_dirs, _) = walk(&meta)?;
    for dir in data_dirs.into_iter().chain(meta_dirs).rev() {
        let _ = std::fs::remove_dir(dir);
    }
    Ok(removed)
}
pub(super) fn staging(root: &Path) -> Result<(), String> {
    if !regular(root, true)? {
        return Ok(());
    }
    for row in std::fs::read_dir(root).map_err(|_| "Временный буфер недоступен")?
    {
        let row = row.map_err(|_| "Временный буфер недоступен")?;
        let name = row.file_name();
        let Some(stem) = name.to_str().and_then(|s| s.strip_suffix(".upload")) else {
            continue;
        };
        if uuid::Uuid::parse_str(stem).is_ok() && regular(&row.path(), false)? {
            std::fs::remove_file(row.path())
                .map_err(|_| "Не удалось очистить временную копию передачи")?;
        }
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn contains(cache: &Path, name: &str) -> bool {
    walk(&cache.join("vfs"))
        .unwrap()
        .1
        .iter()
        .any(|p| p.file_name().is_some_and(|n| n == name))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_confirmed_clean_files_are_removed() {
        let root =
            std::env::temp_dir().join(format!("cloudloom-clean-test-{}", uuid::Uuid::new_v4()));
        let meta = root.join("vfsMeta/storage");
        let data = root.join("vfs/storage");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("complete.mp4"), b"uploaded").unwrap();
        std::fs::write(meta.join("complete.mp4"), br#"{"Dirty":false}"#).unwrap();
        std::fs::write(data.join("pending.mp4"), b"must survive").unwrap();
        std::fs::write(meta.join("pending.mp4"), br#"{"Dirty":true}"#).unwrap();
        assert!(clean(&root).is_err());
        assert!(data.join("complete.mp4").exists());
        assert!(data.join("pending.mp4").exists());
        std::fs::write(meta.join("pending.mp4"), br#"{"Dirty":false}"#).unwrap();
        std::fs::write(data.join("unknown.mp4"), b"no metadata: preserve").unwrap();
        assert_eq!(clean(&root).unwrap(), 2);
        assert!(data.join("unknown.mp4").exists());
        let stage = root.join("delivery-uploads");
        std::fs::create_dir(&stage).unwrap();
        let orphan = stage.join(format!("{}.upload", uuid::Uuid::new_v4()));
        std::fs::write(&orphan, b"staging duplicate").unwrap();
        std::fs::write(stage.join("keep.upload"), b"unknown").unwrap();
        staging(&stage).unwrap();
        assert!(!orphan.exists());
        assert!(stage.join("keep.upload").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
