//! Run with the repository assets available and PRODUCT=nova MODEL_NUMBER=376.
use plato_core::{
    battery::FakeBattery,
    context::Context,
    font::Fonts,
    framebuffer::{Framebuffer, Pixmap},
    frontlight::LightLevels,
    library::Library,
    settings::{LibraryMode, Settings},
    view::{self, Bus, EntryId, Event, RenderQueue, View, ViewId},
    view::{gallery::Gallery, key::KeyKind, notes::Notes, preferences::Preferences},
};
use std::{collections::VecDeque, sync::mpsc};

fn dispatch(view: &mut dyn View, event: Event, context: &mut Context) {
    let (tx, rx) = mpsc::channel();
    let mut queue = VecDeque::from([event]);
    let mut rq = RenderQueue::new();
    let mut count = 0;
    while let Some(event) = queue.pop_front() {
        count += 1;
        assert!(count < 100, "event loop");
        let mut bus = Bus::new();
        view::handle_event(view, &event, &tx, &mut bus, &mut rq, context);
        queue.extend(bus);
        queue.extend(rx.try_iter());
    }
}
fn paint(view: &dyn View, context: &mut Context) {
    view.render(context.fb.as_mut(), *view.rect(), &mut context.fonts);
    for child in view.children() {
        paint(child.as_ref(), context);
    }
}
fn snapshot(view: &dyn View, context: &mut Context, name: &str) {
    if view.view_id() == Some(ViewId::FileActions) {
        if let Some(keyboard) = view
            .children()
            .iter()
            .find(|v| v.is::<view::keyboard::Keyboard>())
        {
            for child in view.children().iter().filter(|v| {
                v.is::<view::button::Button>() || v.is::<view::input_field::InputField>()
            }) {
                assert!(
                    !child.rect().overlaps(keyboard.rect()),
                    "file controls must stay above the keyboard"
                );
            }
        }
    }
    for child in view.children() {
        if child.view_id() == Some(ViewId::FileActions) {
            if let Some(keyboard) = child
                .children()
                .iter()
                .find(|v| v.is::<view::keyboard::Keyboard>())
            {
                for control in child.children().iter().filter(|v| {
                    v.is::<view::button::Button>() || v.is::<view::input_field::InputField>()
                }) {
                    assert!(
                        !control.rect().overlaps(keyboard.rect()),
                        "file controls must stay above the keyboard"
                    );
                }
            }
        }
    }
    paint(view, context);
    if let Ok(folder) = std::env::var("CAESAR_TEST_OUTPUT") {
        std::fs::create_dir_all(&folder).unwrap();
        context
            .fb
            .save(&format!("{}/{}.png", folder, name))
            .unwrap();
    }
}
fn select(view: &mut dyn View, id: EntryId, context: &mut Context) {
    dispatch(view, Event::Select(id), context);
}
fn replace_name(view: &mut dyn View, name: &str, context: &mut Context) {
    dispatch(
        view,
        Event::Select(EntryId::SetInputText(ViewId::FileNameInput, name.into())),
        context,
    );
}
#[test]
fn apps_file_actions_preserve_notes_and_refresh_lists_in_both_orientations() {
    let assets = std::env::var_os("CAESAR_TEST_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    std::env::set_current_dir(assets).unwrap();
    for (width, height) in [(1072, 1448), (1448, 1072)] {
        let root = std::env::temp_dir().join(format!("caesar-ui-{}-{}", std::process::id(), width));
        std::fs::create_dir_all(root.join("Notes")).unwrap();
        std::fs::create_dir_all(root.join("Screenshots")).unwrap();
        let mut settings = Settings::default();
        settings.libraries[0].path = root.clone();
        settings.reader.font_path = "fonts".into();
        let library = Library::new(&root, LibraryMode::Filesystem).unwrap();
        let fonts = Fonts::load().unwrap();
        let mut context = Context::new(
            Box::new(Pixmap::new(width, height, 1)),
            None,
            library,
            settings,
            fonts,
            Box::new(FakeBattery::new()),
            Box::new(LightLevels::default()),
            Box::new(0u16),
        );
        context.load_keyboard_layouts();
        let rect = context.fb.rect();
        let mut rq = RenderQueue::new();
        let image = root.join("Screenshots/screenshot-fixture.png");
        Pixmap::new(200, 200, 1)
            .save(image.to_str().unwrap())
            .unwrap();
        let mut gallery = Gallery::new(rect, &mut rq, &mut context);
        select(&mut gallery, EntryId::GalleryFile(0), &mut context);
        select(&mut gallery, EntryId::FileRename, &mut context);
        let taken = root.join("Screenshots/taken.png");
        std::fs::write(&taken, b"preserve").unwrap();
        replace_name(&mut gallery, "taken", &mut context);
        select(&mut gallery, EntryId::FileRenameSave, &mut context);
        assert_eq!(std::fs::read(&taken).unwrap(), b"preserve");
        assert!(image.exists());
        snapshot(&gallery, &mut context, &format!("rename-error-{}", width));
        std::fs::remove_file(taken).unwrap();
        replace_name(&mut gallery, "Reading notes", &mut context);
        // A real keyboard output must reach the focused filename field.
        dispatch(&mut gallery, Event::Key(KeyKind::Output('!')), &mut context);
        snapshot(&gallery, &mut context, &format!("rename-{}", width));
        select(&mut gallery, EntryId::FileRenameSave, &mut context);
        let renamed = root.join("Screenshots/Reading notes!.png");
        assert!(renamed.exists());
        assert!(!image.exists());
        select(&mut gallery, EntryId::GalleryOpen(0), &mut context);
        snapshot(&gallery, &mut context, &format!("gallery-{}", width));
        select(&mut gallery, EntryId::GalleryFile(0), &mut context);
        select(&mut gallery, EntryId::FileDelete, &mut context);
        dispatch(&mut gallery, Event::Back, &mut context);
        assert!(renamed.exists());
        select(&mut gallery, EntryId::GalleryFile(0), &mut context);
        select(&mut gallery, EntryId::FileDelete, &mut context);
        snapshot(&gallery, &mut context, &format!("delete-{}", width));
        select(&mut gallery, EntryId::FileDeleteConfirm, &mut context);
        assert!(!renamed.exists());
        let note = root.join("Notes/note-fixture.md");
        std::fs::write(&note, "# Original\nKeep this text.").unwrap();
        let mut notes = Notes::new(rect, &mut rq, &mut context);
        select(&mut notes, EntryId::NoteOpen(0), &mut context);
        dispatch(
            &mut notes,
            Event::Keyboard(view::KeyboardEvent::Append('X')),
            &mut context,
        );
        snapshot(&notes, &mut context, &format!("notes-{}", width));
        select(&mut notes, EntryId::NoteFile(usize::MAX), &mut context);
        select(&mut notes, EntryId::FileRename, &mut context);
        replace_name(&mut notes, "Trip plan", &mut context);
        select(&mut notes, EntryId::FileRenameSave, &mut context);
        let renamed = root.join("Notes/Trip plan.md");
        assert!(renamed.exists());
        assert!(!note.exists());
        assert!(std::fs::read_to_string(&renamed).unwrap().contains('X'));
        dispatch(&mut notes, Event::Back, &mut context);
        select(&mut notes, EntryId::NoteOpen(0), &mut context);
        select(&mut notes, EntryId::NoteFile(usize::MAX), &mut context);
        select(&mut notes, EntryId::FileDelete, &mut context);
        select(&mut notes, EntryId::FileDeleteConfirm, &mut context);
        dispatch(&mut notes, Event::Save, &mut context);
        assert!(
            !renamed.exists(),
            "deleted note must not be recreated by autosave"
        );
        let mut prefs = Preferences::new(rect, &mut rq, &mut context);
        select(&mut prefs, EntryId::SettingsSection(0), &mut context);
        snapshot(&prefs, &mut context, &format!("settings-{}", width));
        // Clara HD has a warmth row, so regular refresh is index 5.
        select(&mut prefs, EntryId::SettingsChoose(5), &mut context);
        snapshot(&prefs, &mut context, &format!("refresh-{}", width));
        std::fs::remove_dir_all(root).unwrap();
    }
}
