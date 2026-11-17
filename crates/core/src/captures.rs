//! Storage shared by the device, desktop emulator, and capture gallery.
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{bail, Context as _, Error};
use chrono::Local;
use walkdir::WalkDir;
use crate::context::Context;
use crate::helpers::IsHidden;

pub fn save_screenshot(context: &Context) -> Result<PathBuf, Error> {
    let directory = context.library.home.join(&context.settings.screenshot_path);
    fs::create_dir_all(&directory)?;
    let path = directory.join(Local::now().format("screenshot-%Y%m%d_%H%M%S_%f.png").to_string());
    context.fb.save(&path.to_string_lossy())?;
    Ok(path)
}

// `legacy` roots are searched only at the top level for screenshot-*.png.
// Files stay in place, and overlapping roots must not produce duplicate entries.
pub fn list_images(roots: &[(PathBuf, bool)]) -> Result<Vec<PathBuf>, Error> {
    let mut files: BTreeMap<PathBuf, SystemTime> = BTreeMap::new();
    for (root, legacy) in roots {
        match fs::metadata(root) {
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            result => {
                if !result.with_context(|| format!("Can't read {}", root.display()))?.is_dir() {
                    bail!("Not a folder: {}", root.display());
                }
            },
        }
        for entry in WalkDir::new(root).min_depth(1)
            .max_depth(if *legacy { 1 } else { usize::MAX })
            .into_iter().filter_entry(|e| e.depth() == 0 || !e.is_hidden()) {
            let entry = entry.with_context(|| format!("Can't browse {}", root.display()))?;
            if !entry.file_type().is_file() || !is_png(entry.path()) {
                continue;
            }
            if *legacy && !entry.file_name().to_string_lossy().starts_with("screenshot-") {
                continue;
            }
            let path = fs::canonicalize(entry.path())?;
            let modified = entry.metadata()?.modified().unwrap_or(UNIX_EPOCH);
            files.insert(path, modified);
        }
    }
    let mut files: Vec<_> = files.into_iter().collect();
    files.sort_by(|(a, ta), (b, tb)| tb.cmp(ta).then_with(|| b.cmp(a)));
    Ok(files.into_iter().map(|(path, _)| path).collect())
}

fn is_png(path: &Path) -> bool {
    path.extension().and_then(|v| v.to_str()).map_or(false, |v| v.eq_ignore_ascii_case("png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDirectory(PathBuf);
    impl TempDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("caesar-gallery-{}-{}",
                std::process::id(), Local::now().timestamp_nanos_opt().unwrap()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDirectory {
        fn drop(&mut self) { fs::remove_dir_all(&self.0).ok(); }
    }

    #[test]
    fn discovers_new_and_legacy_captures_without_duplicates_or_moving_files() {
        let temp = TempDirectory::new();
        let screenshots = temp.0.join("Screenshots");
        fs::create_dir_all(screenshots.join("nested")).unwrap();
        fs::create_dir_all(screenshots.join(".hidden")).unwrap();
        let expected = [temp.0.join("screenshot-old.png"), screenshots.join("screenshot-new.png"),
                        screenshots.join("nested/sketch.PNG")];
        for path in &expected { fs::write(path, b"fixture").unwrap(); }
        for path in [temp.0.join("book-cover.png"), screenshots.join("notes.txt"),
                     screenshots.join(".hidden/secret.png")] {
            fs::write(path, b"fixture").unwrap();
        }
        let mut actual = list_images(&[(screenshots.clone(), false), (temp.0.clone(), true),
                                      (screenshots, false), (temp.0.join("missing"), false)]).unwrap();
        actual.sort();
        let mut expected = expected.to_vec();
        expected.sort();
        assert_eq!(actual, expected);
        assert!(actual.iter().all(|p| p.exists()));
    }

    #[test]
    fn orders_images_by_newest_first_and_reports_unreadable_roots() {
        let temp = TempDirectory::new();
        let older = temp.0.join("z.png");
        let newer = temp.0.join("a.png");
        for (path, time) in [(&older, 100), (&newer, 200)] {
            let file = fs::File::create(path).unwrap();
            file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(time)).unwrap();
        }
        assert_eq!(list_images(&[(temp.0.clone(), false)]).unwrap(), vec![newer, older]);
        let invalid = temp.0.join("invalid");
        fs::write(&invalid, b"not a directory").unwrap();
        assert!(list_images(&[(invalid.clone(), false)]).is_err());
        assert!(list_images(&[(invalid.join("child"), false)]).is_err());
    }

    #[test]
    fn old_settings_keep_the_default_screenshot_folder() {
        let settings: crate::settings::Settings = toml::from_str("").unwrap();
        assert_eq!(settings.screenshot_path, PathBuf::from("Screenshots"));
        let settings: crate::settings::Settings = toml::from_str("screenshot-path = '/custom/captures'").unwrap();
        assert_eq!(settings.screenshot_path, PathBuf::from("/custom/captures"));
    }
}
