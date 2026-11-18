//! Run with the repository assets available and PRODUCT=nova MODEL_NUMBER=376.
use plato_core::{
    battery::FakeBattery,
    context::Context,
    font::Fonts,
    geom::{CycleDir, Point},
    gesture::GestureEvent,
    framebuffer::{Framebuffer, Pixmap},
    frontlight::LightLevels,
    library::Library,
    settings::{controls::{Action, Binding}, LibraryMode, Settings},
    view::{self, Bus, EntryId, Event, RenderQueue, View, ViewId},
    view::{button::Button, gallery::Gallery, icon::Icon, key::KeyKind, menu::Menu, menu_entry::MenuEntry, notes::Notes, preferences::Preferences},
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
fn apps_support_file_actions_and_grouped_settings_in_both_orientations() {
    let assets = std::env::var_os("CAESAR_TEST_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    std::env::set_current_dir(&assets).unwrap();
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
        let image_row = button_center(&gallery, 3);
        let left = Point::new(width as i32 / 5, image_row.y);
        let right = Point::new(width as i32 - 30, image_row.y + 20);
        hold(&mut gallery, left, &mut context);
        let popup = file_actions(&gallery).unwrap();
        let left_popup = *popup.rect();
        assert!(left_popup.width() < rect.width() / 2);
        assert!(left_popup.height() < rect.height() / 2);
        assert!(left_popup.includes(left));
        assert!(popup.child(0).is::<Menu>());
        snapshot(&gallery, &mut context, &format!("file-popup-{}", width));
        tap(&mut gallery, Point::new(rect.min.x + 1, rect.min.y + 1), &mut context);
        assert!(!has_file_actions(&gallery));
        assert!(image.exists());
        hold(&mut gallery, right, &mut context);
        let popup = file_actions(&gallery).unwrap();
        assert!(popup.rect().min.x > left_popup.min.x);
        assert_eq!(popup.rect().min.y - left_popup.min.y, right.y - left.y);
        assert!(rect.contains(popup.rect()));
        assert!(popup.rect().includes(right));
        let rename = popup.child(0).child(0).rect().center();
        tap(&mut gallery, rename, &mut context);
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
        tap(&mut gallery, image_row, &mut context);
        snapshot(&gallery, &mut context, &format!("gallery-{}", width));
        let lower_press = Point::new(width as i32 / 3, height as i32 - 150);
        hold(&mut gallery, lower_press, &mut context);
        let popup = file_actions(&gallery).unwrap();
        assert!(popup.rect().includes(lower_press));
        assert!(popup.rect().center().y < lower_press.y);
        select(&mut gallery, EntryId::FileDelete, &mut context);
        dispatch(&mut gallery, Event::Back, &mut context);
        assert!(renamed.exists());
        hold(&mut gallery, rect.center(), &mut context);
        select(&mut gallery, EntryId::FileDelete, &mut context);
        snapshot(&gallery, &mut context, &format!("delete-{}", width));
        select(&mut gallery, EntryId::FileDeleteConfirm, &mut context);
        assert!(!renamed.exists());

        for name in ["screenshot-first.png", "screenshot-second.png"] {
            Pixmap::new(20, 20, 1).save(root.join("Screenshots").join(name).to_str().unwrap()).unwrap();
        }
        select(&mut gallery, EntryId::Refresh, &mut context);
        select(&mut gallery, EntryId::GalleryOpen(0), &mut context);
        let next = arrow_center(&gallery, CycleDir::Next).unwrap();
        assert!(arrow_center(&gallery, CycleDir::Previous).is_none());
        dispatch(&mut gallery, Event::Gesture(GestureEvent::Tap(next)), &mut context);
        assert!(arrow_center(&gallery, CycleDir::Previous).is_some());
        assert!(arrow_center(&gallery, CycleDir::Next).is_none());
        snapshot(&gallery, &mut context, &format!("gallery-pager-{}", width));

        let note = root.join("Notes/note-fixture.md");
        std::fs::write(&note, "# Original\nKeep this text.").unwrap();
        let mut notes = Notes::new(rect, &mut rq, &mut context);
        let note_row = button_center(&notes, 2);
        hold(&mut notes, note_row, &mut context);
        assert!(has_file_actions(&notes));
        dispatch(&mut notes, Event::Back, &mut context);
        tap(&mut notes, note_row, &mut context);
        dispatch(
            &mut notes,
            Event::Keyboard(view::KeyboardEvent::Append('X')),
            &mut context,
        );
        snapshot(&notes, &mut context, &format!("notes-{}", width));
        let note_title = notes.child(0).child(1).rect().center();
        hold(&mut notes, note_title, &mut context);
        assert!(has_file_actions(&notes));
        select(&mut notes, EntryId::FileRename, &mut context);
        replace_name(&mut notes, "Trip plan", &mut context);
        select(&mut notes, EntryId::FileRenameSave, &mut context);
        let renamed = root.join("Notes/Trip plan.md");
        assert!(renamed.exists());
        assert!(!note.exists());
        assert!(std::fs::read_to_string(&renamed).unwrap().contains('X'));
        dispatch(&mut notes, Event::Back, &mut context);
        hold(&mut notes, note_row, &mut context);
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

        // Keep UI writes to Settings.toml inside the disposable fixture.
        std::env::set_current_dir(&root).unwrap();
        let dark_mode = button_center(&prefs, 3);
        tap(&mut prefs, dark_mode, &mut context);
        assert!(context.settings.inverted);
        let saved: Settings = toml::from_str(&std::fs::read_to_string("Settings.toml").unwrap()).unwrap();
        assert!(saved.inverted);
        tap(&mut prefs, dark_mode, &mut context);
        assert!(!context.settings.inverted);
        select(&mut prefs, EntryId::SettingsSection(1), &mut context);
        let clock_format = button_center(&prefs, 4);
        tap(&mut prefs, clock_format, &mut context);
        assert_eq!(context.settings.time_format, "%I:%M %p");
        tap(&mut prefs, clock_format, &mut context);
        assert_eq!(context.settings.time_format, "%H:%M");
        std::env::set_current_dir(&assets).unwrap();

        select(&mut prefs, EntryId::SettingsSection(0), &mut context);
        // Clara HD has a warmth row, so regular refresh is index 5.
        select(&mut prefs, EntryId::SettingsChoose(5), &mut context);
        assert_eq!(context.settings.reader.refresh_rate.global.regular, 8);
        snapshot(&prefs, &mut context, &format!("refresh-{}", width));
        dispatch(&mut prefs, Event::Back, &mut context);
        dispatch(&mut prefs, Event::Back, &mut context);

        select(&mut prefs, EntryId::SettingsSection(4), &mut context);
        snapshot(&prefs, &mut context, &format!("reading-controls-{}", width));
        select(&mut prefs, EntryId::SettingsGroup(3), &mut context);
        if width > height {
            let next = arrow_center(&prefs, CycleDir::Next).unwrap();
            assert!(arrow_center(&prefs, CycleDir::Previous).is_none());
            dispatch(&mut prefs, Event::Gesture(GestureEvent::Tap(next)), &mut context);
            let previous = arrow_center(&prefs, CycleDir::Previous).unwrap();
            assert!(arrow_center(&prefs, CycleDir::Next).is_none());
            dispatch(&mut prefs, Event::Gesture(GestureEvent::Tap(previous)), &mut context);
            assert!(arrow_center(&prefs, CycleDir::Next).is_some());
        }
        dispatch(&mut prefs, Event::Back, &mut context);
        select(&mut prefs, EntryId::SettingsGroup(0), &mut context);
        snapshot(&prefs, &mut context, &format!("tap-bindings-{}", width));

        select(&mut prefs, EntryId::SettingsChoose(5), &mut context);
        snapshot(&prefs, &mut context, &format!("change-binding-{}", width));
        select(&mut prefs, EntryId::SettingsActions(3), &mut context);
        snapshot(&prefs, &mut context, &format!("action-group-{}", width));

        // Keep UI writes to Settings.toml inside the disposable fixture.
        std::env::set_current_dir(&root).unwrap();
        select(&mut prefs, EntryId::SettingsApply(1), &mut context);
        let saved: Settings = toml::from_str(&std::fs::read_to_string("Settings.toml").unwrap()).unwrap();
        assert_eq!(saved.controls.action(Binding::TapEast), Action::Notes);
        snapshot(&prefs, &mut context, &format!("changed-binding-{}", width));
        select(&mut prefs, EntryId::SettingsChoose(5), &mut context);
        select(&mut prefs, EntryId::SettingsApply(0), &mut context);
        assert!(!context.settings.controls.bindings.contains_key(&Binding::TapEast));
        std::env::set_current_dir(&assets).unwrap();

        let menu_button = prefs.child(0).child(5).rect().center();
        tap(&mut prefs, menu_button, &mut context);
        let main_menu = prefs.children().iter().find(|child| child.view_id() == Some(ViewId::MainMenu)).unwrap();
        let entries: Vec<_> = main_menu.children().iter().filter(|child| child.is::<MenuEntry>()).collect();
        assert_eq!(entries.len(), 9);
        let applications = entries[6].rect().center();
        tap(&mut prefs, applications, &mut context);
        let main_menu = prefs.children().iter().find(|child| child.view_id() == Some(ViewId::MainMenu)).unwrap();
        let apps = main_menu.children().iter().find(|child| child.is::<Menu>()).unwrap();
        assert_eq!(apps.len(), 7);
        assert!(apps.children().iter().all(|child| child.is::<MenuEntry>()));
        assert!(rect.contains(apps.rect()));
        assert!(apps.child(0).rect().min.y < apps.child(6).rect().min.y);
        snapshot(&prefs, &mut context, &format!("applications-{}", width));
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn arrow_center(view: &dyn View, direction: CycleDir) -> Option<Point> {
    view.children().iter().find_map(|child| {
        let icon = child.downcast_ref::<Icon>()?;
        (icon.name == Icon::navigation_name(direction)).then(|| {
            let rect = icon.rect();
            Point::new((rect.min.x + rect.max.x) / 2, (rect.min.y + rect.max.y) / 2)
        })
    })
}

fn button_center(view: &dyn View, index: usize) -> Point {
    view.children().iter().filter(|child| child.is::<Button>()).nth(index).unwrap().rect().center()
}

fn has_file_actions(view: &dyn View) -> bool {
    file_actions(view).is_some()
}

fn file_actions(view: &dyn View) -> Option<&dyn View> {
    view.children().iter().find(|child| child.view_id() == Some(ViewId::FileActions))
        .map(Box::as_ref)
}

fn tap(view: &mut dyn View, point: Point, context: &mut Context) {
    dispatch(view, Event::Gesture(GestureEvent::Tap(point)), context);
}

fn hold(view: &mut dyn View, point: Point, context: &mut Context) {
    dispatch(view, Event::Gesture(GestureEvent::HoldFingerShort(point, 0)), context);
}
