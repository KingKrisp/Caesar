//! Plain Markdown note storage. Files are independent of the book database.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{bail, Context as _, Error};
use chrono::Local;

pub const MAX_NOTE_BYTES: usize = 256 * 1024;

pub struct NoteSummary {
    pub path: PathBuf,
    pub title: String,
    modified: SystemTime,
}

#[derive(Default)]
pub struct NoteFile {
    pub path: Option<PathBuf>,
    pub saved_text: String,
}

impl NoteFile {
    pub fn load(path: &Path) -> Result<Self, Error> {
        Ok(Self { path: Some(path.to_path_buf()), saved_text: read_note(path)? })
    }

    pub fn save(&mut self, directory: &Path, text: &str) -> Result<(), Error> {
        if text == self.saved_text || (self.path.is_none() && text.trim().is_empty()) {
            return Ok(());
        }
        if text.len() > MAX_NOTE_BYTES {
            bail!("This note exceeds the 256 KiB editor limit. Your edits are still open.");
        }
        if let Some(path) = &self.path {
            if read_note(path)? != self.saved_text {
                bail!("This file changed outside Notes. Your edits are still open; the file was not overwritten.");
            }
        }
        fs::create_dir_all(directory)?;
        let stamp = Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
        let path = self.path.clone().unwrap_or_else(|| directory.join(format!("note-{}.md", stamp)));
        if self.path.is_none() && path.exists() {
            bail!("A note with this filename already exists. Please save again.");
        }
        // Same-directory rename keeps the previous complete file until the write succeeds.
        let parent = path.parent().unwrap_or(directory);
        let temporary = parent.join(format!(".note-{}.tmp", stamp));
        let mut file = OpenOptions::new().create_new(true).write(true).open(&temporary)?;
        let result = (|| -> Result<(), Error> {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, &path)?;
            Ok(())
        })();
        if result.is_err() { fs::remove_file(&temporary).ok(); }
        result?;
        self.path = Some(path);
        self.saved_text = text.to_string();
        Ok(())
    }
}

fn read_note(path: &Path) -> Result<String, Error> {
    let mut text = String::new();
    File::open(path)?.take(MAX_NOTE_BYTES as u64 + 1).read_to_string(&mut text)
        .with_context(|| format!("Can't read {} as UTF-8 Markdown", path.display()))?;
    if text.len() > MAX_NOTE_BYTES { bail!("This note is too large for the editor (256 KiB limit)."); }
    Ok(text)
}

pub fn note_title(text: &str) -> String {
    text.lines().find(|line| !line.trim().is_empty())
        .map(|line| line.trim().trim_start_matches('#').trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.chars().take(120).collect())
        .unwrap_or_else(|| "Untitled note".to_string())
}

pub fn list_notes(directory: &Path) -> Result<Vec<NoteSummary>, Error> {
    let entries = match fs::read_dir(directory) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        other => other?,
    };
    let mut notes = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() || entry.file_name().to_string_lossy().starts_with('.') ||
           !path.extension().and_then(|v| v.to_str()).map_or(false, |v| v.eq_ignore_ascii_case("md")) {
            continue;
        }
        let mut prefix = Vec::new();
        File::open(&path)?.take(4096).read_to_end(&mut prefix)?;
        let title = note_title(&String::from_utf8_lossy(&prefix));
        notes.push(NoteSummary { path, title, modified: entry.metadata()?.modified().unwrap_or(UNIX_EPOCH) });
    }
    notes.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.path.cmp(&a.path)));
    Ok(notes)
}

pub fn preview_html(markdown: &str) -> String {
    use pulldown_cmark::{Parser, Event, html};
    // Notes are text: raw HTML is displayed literally rather than interpreted.
    let parser = Parser::new(markdown).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        other => other,
    });
    let mut html = String::new();
    html::push_html(&mut html, parser);
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("caesar-notes-{}-{}", std::process::id(),
                Local::now().timestamp_nanos_opt().unwrap()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp { fn drop(&mut self) { fs::remove_dir_all(&self.0).ok(); } }

    #[test]
    fn markdown_survives_create_edit_and_reopen_without_duplicate_files() {
        let temp = Temp::new();
        let folder = temp.0.join("Notes");
        let mut note = NoteFile::default();
        note.save(&folder, " \n").unwrap();
        assert!(!folder.exists());
        let text = "# Café\n\nSome **bold**, *italic*, and 日本語.\n";
        note.save(&folder, text).unwrap();
        let path = note.path.clone().unwrap();
        let mut reopened = NoteFile::load(&path).unwrap();
        assert_eq!(reopened.saved_text, text);
        reopened.save(&folder, "# Renamed title\nA new line\n").unwrap();
        assert_eq!(reopened.path.as_ref(), Some(&path));
        let notes = list_notes(&folder).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Renamed title");
        assert_eq!(fs::read_to_string(path).unwrap(), "# Renamed title\nA new line\n");
        reopened.save(&folder, "").unwrap();
        assert_eq!(fs::read_to_string(reopened.path.unwrap()).unwrap(), "");
    }

    #[test]
    fn failed_or_conflicting_saves_preserve_previous_content() {
        let temp = Temp::new();
        let mut note = NoteFile::default();
        note.save(&temp.0, "original").unwrap();
        let path = note.path.clone().unwrap();
        assert!(note.save(&temp.0, &"x".repeat(MAX_NOTE_BYTES + 1)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        fs::write(&path, "changed on computer").unwrap();
        assert!(note.save(&temp.0, "edits on reader").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "changed on computer");
        assert_eq!(note.saved_text, "original");
        let blocked = temp.0.join("blocked");
        fs::write(&blocked, "file, not folder").unwrap();
        let mut fresh = NoteFile::default();
        assert!(fresh.save(&blocked, "draft").is_err());
        assert!(fresh.path.is_none());
    }

    #[test]
    fn notes_list_is_newest_first_and_ignores_other_files_and_symlinks() {
        let temp = Temp::new();
        for (name, text, time) in [("older.md", "# Older", 100), ("newer.MD", "# Newer", 200),
                                   (".hidden.md", "hidden", 300), ("book.txt", "book", 400)] {
            let path = temp.0.join(name);
            fs::write(&path, text).unwrap();
            File::open(path).unwrap().set_modified(UNIX_EPOCH + std::time::Duration::from_secs(time)).unwrap();
        }
        std::os::unix::fs::symlink(temp.0.join("older.md"), temp.0.join("link.md")).unwrap();
        let notes = list_notes(&temp.0).unwrap();
        assert_eq!(notes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), ["Newer", "Older"]);
        assert!(list_notes(&temp.0.join("missing")).unwrap().is_empty());
    }

    #[test]
    fn preview_renders_basic_markdown_and_escapes_raw_html() {
        let html = preview_html("# Heading\n\n**bold** and *italic* <script>bad()</script>\n");
        assert!(html.contains("<h1>Heading</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn existing_settings_default_to_notes_folder_and_allow_an_absolute_override() {
        let settings: crate::settings::Settings = toml::from_str("").unwrap();
        assert_eq!(settings.notes_path, PathBuf::from("Notes"));
        let settings: crate::settings::Settings = toml::from_str("notes-path = '/custom/notes'").unwrap();
        assert_eq!(settings.notes_path, PathBuf::from("/custom/notes"));
    }
}
