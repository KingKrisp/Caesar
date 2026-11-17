//! File actions shared by Notes and Gallery.
use anyhow::{bail, Error};
use std::fs;
use std::path::{Path, PathBuf};

fn regular_file(path: &Path) -> Result<(), Error> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        bail!("Select a regular file.");
    }
    Ok(())
}

/// Rename a file without changing its type or replacing another file.
pub fn rename(path: &Path, directory: &Path, name: &str) -> Result<PathBuf, Error> {
    regular_file(path)?;
    let name = name.trim();
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with('.')
        || name.len() > 180
        || name
            .chars()
            .any(|c| c.is_control() || "/\\<>:\"|?*".contains(c))
    {
        bail!("Use a name without slashes or special characters.");
    }
    let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
    let filename = if extension.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", name, extension)
    };
    fs::create_dir_all(directory)?;
    let target = directory.join(filename);
    if target == path {
        return Ok(target);
    }
    // Kobo storage is exclusively owned by the application until USB sharing.
    if fs::symlink_metadata(&target).is_ok() {
        bail!("A file with that name already exists.");
    }
    fs::rename(path, &target)?;
    Ok(target)
}

pub fn delete(path: &Path) -> Result<(), Error> {
    regular_file(path)?;
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_actions_preserve_contents_and_reject_overwrites_and_invalid_names() {
        let root = std::env::temp_dir().join(format!(
            "caesar-files-{}-{}",
            std::process::id(),
            chrono::Local::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("original.PNG");
        fs::write(&source, b"image").unwrap();
        fs::write(root.join("taken.PNG"), b"other").unwrap();
        for name in [
            "taken",
            "../escape",
            "/escape",
            "",
            ".hidden",
            "bad:name",
            "trailing.",
        ] {
            assert!(rename(&source, &root, name).is_err());
            assert_eq!(fs::read(&source).unwrap(), b"image");
        }
        let new = rename(&source, &root, "Café sketch").unwrap();
        assert_eq!(new.file_name().unwrap(), "Café sketch.PNG");
        assert_eq!(fs::read(&new).unwrap(), b"image");
        assert!(!source.exists());
        assert!(delete(&root).is_err());
        delete(&new).unwrap();
        assert!(!new.exists());
        assert_eq!(fs::read(root.join("taken.PNG")).unwrap(), b"other");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_capture_can_be_named_and_kept_in_the_gallery() {
        let root = std::env::temp_dir().join(format!(
            "caesar-legacy-{}-{}",
            std::process::id(),
            chrono::Local::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let old = root.join("screenshot-old.png");
        fs::write(&old, b"image").unwrap();
        let folder = root.join("Screenshots");
        let new = rename(&old, &folder, "Reading notes").unwrap();
        assert_eq!(
            crate::captures::list_images(&[(folder, false)]).unwrap(),
            vec![new]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
