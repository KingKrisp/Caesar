use std::path::Path;
use crate::color::WHITE;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{CycleDir, Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::settings::{SETTINGS_PATH, RotationLock};
use crate::settings::controls::{Action, Binding};
use crate::settings::preferences::{self, Preference, Value};
use crate::unit::scale_by_dpi;
use super::{Align, Bus, EntryId, Event, Hub, Id, ID_FEEDER, RenderData, RenderQueue, View, ViewId, SMALL_BAR_HEIGHT};
use super::button::Button;
use super::label::Label;
use super::notification::Notification;
use super::top_bar::TopBar;
use super::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};

const SECTIONS: &[&str] = &["Display and light", "Sleep, clock and USB", "Reading defaults", "Library",
                           "Reading gestures", "Global shortcuts", "Device and storage"];

pub struct Preferences {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    section: Option<u8>,
    choice: Option<Preference>,
    page: usize,
    return_page: usize,
}

fn numbers(values: &[f32]) -> Vec<(String, Value)> {
    values.iter().map(|v| (v.to_string(), Value::Number(*v))).collect()
}

fn texts(values: &[&str]) -> Vec<(String, Value)> {
    values.iter().map(|v| (v.to_string(), Value::Text(v.to_string()))).collect()
}

impl Preferences {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> Self {
        if context.settings.frontlight { context.settings.frontlight_levels = context.frontlight.levels(); }
        context.settings.inverted = context.fb.inverted();
        let mut view = Self { id: ID_FEEDER.next(), rect, children: Vec::new(),
            section: None, choice: None, page: 0, return_page: 0 };
        view.rebuild(rq, context);
        view
    }

    fn items(&self) -> Vec<Preference> {
        use Preference::*;
        match self.section {
            Some(0) => {
                let mut items = vec![Frontlight, Brightness, Inverted, Rotation, Refresh, RefreshInverted];
                if CURRENT_DEVICE.frontlight_kind() != crate::device::FrontlightKind::Standard { items.insert(2, Warmth); }
                items
            },
            Some(1) => vec![SleepCover, Sleep, PowerOff, AutoShare, TimeFormat, DateFormat, Keyboard],
            Some(2) => vec![FontFamily, FontSize, Margin, LineHeight, Alignment, IgnoreCss, Continuous],
            Some(3) => vec![Thumbnails, FirstColumn, SecondColumn, StartupImport, UsbImport, AddressBar, NavigationBar],
            Some(4) => [StripWidth, CornerWidth, ResetBindings].into_iter()
                .chain(Binding::ALL.iter().filter(|b| !b.is_global()).map(|b| Bind(*b))).collect(),
            Some(5) => Binding::ALL.iter().filter(|b| b.is_global()).map(|b| Bind(*b)).collect(),
            _ => Vec::new(),
        }
    }

    fn options(&self, preference: Preference, context: &Context) -> Vec<(String, Value)> {
        use Preference::*;
        match preference {
            Frontlight | Inverted | SleepCover | AutoShare | IgnoreCss | Continuous | Thumbnails |
            StartupImport | UsbImport | AddressBar | NavigationBar => vec![("Off".into(), Value::Bool(false)), ("On".into(), Value::Bool(true))],
            Brightness | Warmth => numbers(&(0..=10).map(|n| n as f32 * 10.0).collect::<Vec<_>>()),
            Rotation => if CURRENT_DEVICE.has_gyroscope() { texts(&["Automatic", "Portrait", "Landscape"]) }
                        else { texts(&["Portrait", "Landscape"]) },
            Sleep => numbers(&[0., 1., 2., 5., 10., 15., 30., 60.]),
            PowerOff => numbers(&[0., 1., 3., 7., 14.]),
            TimeFormat => vec![("24-hour clock".into(), Value::Text("%H:%M".into())), ("12-hour clock".into(), Value::Text("%I:%M %p".into()))],
            DateFormat => vec![("Long date".into(), Value::Text("%A, %B %-d, %Y".into())),
                              ("YYYY-MM-DD".into(), Value::Text("%Y-%m-%d".into())), ("DD/MM/YYYY".into(), Value::Text("%d/%m/%Y".into()))],
            Keyboard => context.keyboard_layouts.keys().map(|v| (v.clone(), Value::Text(v.clone()))).collect(),
            FontFamily => {
                let mut names = crate::font::family_names("fonts").unwrap_or_default();
                names.extend(crate::font::family_names(&context.settings.reader.font_path).unwrap_or_default());
                names.insert(context.settings.reader.font_family.clone());
                names.into_iter().map(|v| (v.clone(), Value::Text(v))).collect()
            },
            FontSize => numbers(&[6., 7., 8., 9., 10., 11., 12., 13., 14., 16., 18., 20., 24., 28., 32.]),
            Margin => numbers(&[0., 2., 4., 6., 8., 10., 12., 16., 20.]),
            LineHeight => numbers(&[1., 1.1, 1.2, 1.3, 1.4, 1.5, 1.75, 2.]),
            Alignment => texts(&["Left", "Justify", "Center", "Right"]),
            Refresh | RefreshInverted => numbers(&[0., 1., 2., 4., 6., 8., 10., 16., 32.]),
            FirstColumn => texts(&["Title and author", "File name"]),
            SecondColumn => texts(&["Progress", "Year"]),
            StripWidth | CornerWidth => numbers(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]),
            Bind(binding) => Action::ALL.iter().filter(|a| !binding.is_global() || a.is_global())
                .map(|action| (action.label().to_string(), Value::Action(*action))).collect(),
            ResetBindings => texts(&["Restore"]),
        }
    }

    fn rows(&self, context: &mut Context) -> Vec<(String, Option<Event>)> {
        if let Some(preference) = self.choice {
            let current = preference.current(&context.settings);
            return self.options(preference, context).into_iter().enumerate().map(|(index, (mut text, value))| {
                if matches!(value, Value::Number(_)) { text = preference.value_label(&value); }
                (format!("{}{}", if value == current { "✓ " } else { "" }, text), Some(Event::Select(EntryId::SettingsApply(index))))
            }).collect();
        }
        if self.section == Some(6) {
            let mut info = vec![format!("Caesar {}", env!("CARGO_PKG_VERSION")),
                format!("Device: Kobo {}", CURRENT_DEVICE.model),
                format!("Notes: {}", context.library.home.join(&context.settings.notes_path).display()),
                format!("Screenshots: {}", context.library.home.join(&context.settings.screenshot_path).display()),
                format!("Sketches: {}", context.library.home.join(&context.settings.sketch.save_path).display())];
            if let Ok(stats) = nix::sys::statvfs::statvfs(&context.library.home) {
                let free = stats.blocks_available() as f64 * stats.fragment_size() as f64 / 1073741824.0;
                info.insert(2, format!("Free library storage: {:.2} GiB", free));
            }
            if let Ok(capacity) = context.battery.capacity() { info.insert(2, format!("Battery: {:.0}%", capacity[0])); }
            return info.into_iter().map(|text| (text, None)).collect();
        }
        if self.section.is_none() {
            return SECTIONS.iter().enumerate().map(|(i, name)| (name.to_string(), Some(Event::Select(EntryId::SettingsSection(i as u8))))).collect();
        }
        self.items().into_iter().enumerate().map(|(index, preference)| {
            let current = preference.current(&context.settings);
            let value = self.options(preference, context).into_iter().find(|(_, value)| *value == current)
                .map_or_else(|| preference.value_label(&current), |(label, value)| {
                    if matches!(value, Value::Number(_)) { preference.value_label(&value) } else { label }
                });
            (format!("{}: {}", preference.label(), value), Some(Event::Select(EntryId::SettingsChoose(index))))
        }).collect()
    }

    fn header_rows(&self) -> i32 {
        if let Some(preference) = self.choice { 1 + preference.help().len() as i32 }
        else if matches!(self.section, Some(2) | Some(4) | Some(5)) { 2 }
        else { 1 }
    }
    fn per_page(&self) -> usize { (self.rect.height() as i32 / self.bar() - self.header_rows() - 1).max(1) as usize }
    fn bar(&self) -> i32 { scale_by_dpi(SMALL_BAR_HEIGHT, CURRENT_DEVICE.dpi) as i32 }

    fn rebuild(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        self.children.clear();
        let rect = self.rect;
        let bar = self.bar();
        let gap = (bar / 12).max(1);
        let title = self.choice.map(|p| p.label()).unwrap_or_else(|| self.section.map_or("Settings", |s| SECTIONS[s as usize]));
        self.children.push(Box::new(TopBar::new(rect![rect.min.x, rect.min.y, rect.max.x, rect.min.y + bar],
            Event::Back, title.to_string(), context)));
        let descriptions: &[&str] = if let Some(preference) = self.choice { preference.help() }
            else { match self.section {
                Some(2) => &["New book defaults; saved book settings take priority."],
                Some(4) | Some(5) => &["Two-finger center tap always opens Settings."],
                _ => &[],
            }};
        for (line, text) in descriptions.iter().enumerate() {
            let y = rect.min.y + (line as i32 + 1) * bar;
            self.children.push(Box::new(Label::new(rect![rect.min.x,y,rect.max.x,y+bar],(*text).into(),Align::Center)));
        }
        let rows = self.rows(context);
        let pages = (rows.len() + self.per_page() - 1) / self.per_page();
        self.page = self.page.min(pages.saturating_sub(1));
        for (i, (text, event)) in rows.into_iter().skip(self.page * self.per_page()).take(self.per_page()).enumerate() {
            let y = rect.min.y + (i as i32 + self.header_rows()) * bar;
            let row = rect![rect.min.x + gap, y + gap, rect.max.x - gap, y + bar - gap];
            self.children.push(match event {
                Some(event) => Box::new(Button::new(row, event, text)) as Box<dyn View>,
                None => Box::new(Label::new(row, text, Align::Center)) as Box<dyn View>,
            });
        }
        let third = rect.width() as i32 / 3;
        for (text, dir, x, disabled) in [
            ("Previous", CycleDir::Previous, rect.min.x, self.page == 0),
            ("Next", CycleDir::Next, rect.max.x - third, self.page + 1 >= pages),
        ] {
            self.children.push(Box::new(Button::new(rect![x + gap, rect.max.y - bar + gap, x + third - gap, rect.max.y - gap],
                Event::Page(dir), text.to_string()).disabled(disabled)));
        }
        self.children.push(Box::new(Label::new(rect![rect.min.x + third, rect.max.y - bar, rect.max.x - third, rect.max.y],
            format!("{} / {}", if pages == 0 { 0 } else { self.page + 1 }, pages), Align::Center)));
        rq.add(RenderData::new(self.id, rect, UpdateMode::Full));
    }

    fn choose(&mut self, index: usize, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let Some(preference) = self.choice else { return; };
        let Some((_, value)) = self.options(preference, context).into_iter().nth(index) else { return; };
        let mut settings = context.settings.clone();
        if settings.frontlight { settings.frontlight_levels = context.frontlight.levels(); }
        settings.inverted = context.fb.inverted();
        let result = preferences::apply(&mut settings, preference, &value)
            .and_then(|_| preferences::save(&settings, Path::new(SETTINGS_PATH)));
        if let Err(error) = result {
            self.children.push(Box::new(Notification::new(format!("Couldn't save settings: {}", error), hub, rq, context)));
            return;
        }
        context.settings = settings;
        context.fb.set_inverted(context.settings.inverted);
        if context.settings.frontlight {
            context.frontlight.set_intensity(context.settings.frontlight_levels.intensity);
            context.frontlight.set_warmth(context.settings.frontlight_levels.warmth);
        } else {
            context.frontlight.set_intensity(0.0); context.frontlight.set_warmth(0.0);
        }
        if preference == Preference::Rotation {
            let rotation = match context.settings.rotation_lock {
                Some(RotationLock::Landscape) => Some(1), Some(RotationLock::Portrait) => Some(0), _ => None,
            };
            if let Some(rotation) = rotation { hub.send(Event::Select(EntryId::Rotate(CURRENT_DEVICE.from_canonical(rotation)))).ok(); }
        }
        self.choice = None; self.page = self.return_page;
        self.rebuild(rq, context);
    }

    fn turn(&mut self, dir: CycleDir, rq: &mut RenderQueue, context: &mut Context) {
        let pages = (self.rows(context).len() + self.per_page() - 1) / self.per_page();
        self.page = match dir { CycleDir::Previous => self.page.saturating_sub(1),
                               CycleDir::Next => (self.page + 1).min(pages.saturating_sub(1)) };
        self.rebuild(rq, context);
    }
}

impl View for Preferences {
    fn handle_event(&mut self, event: &Event, _hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *event {
            Event::Select(EntryId::SettingsSection(section)) if (section as usize) < SECTIONS.len() => {
                self.section = Some(section); self.choice = None; self.page = 0; self.rebuild(rq, context); true
            },
            Event::Select(EntryId::SettingsChoose(index)) if self.choice.is_none() => {
                if let Some(preference) = self.items().get(index) {
                    self.choice = Some(*preference); self.return_page = self.page; self.page = 0; self.rebuild(rq, context);
                }
                true
            },
            Event::Select(EntryId::SettingsApply(index)) => { self.choose(index, _hub, rq, context); true },
            Event::Back if self.choice.is_some() => {
                self.choice = None; self.page = self.return_page; self.rebuild(rq, context); true
            },
            Event::Back if self.section.is_some() => { self.section = None; self.page = 0; self.rebuild(rq, context); true },
            Event::Reseed => { self.rebuild(rq, context); true },
            Event::Page(dir) => { self.turn(dir, rq, context); true },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::West, .. }) => { self.turn(CycleDir::Next, rq, context); true },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::East, .. }) => { self.turn(CycleDir::Previous, rq, context); true },
            Event::ToggleNear(ViewId::MainMenu, rect) => { toggle_main_menu(self, rect, None, rq, context); true },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => { toggle_battery_menu(self, rect, None, rq, context); true },
            Event::ToggleNear(ViewId::ClockMenu, rect) => { toggle_clock_menu(self, rect, None, rq, context); true },
            _ => false,
        }
    }
    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) { fb.draw_rectangle(&rect, WHITE); }
    fn resize(&mut self, rect: Rectangle, _hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.rect = rect; self.rebuild(rq, context);
    }
    fn is_background(&self) -> bool { true }
    fn rect(&self) -> &Rectangle { &self.rect }
    fn rect_mut(&mut self) -> &mut Rectangle { &mut self.rect }
    fn children(&self) -> &Vec<Box<dyn View>> { &self.children }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> { &mut self.children }
    fn id(&self) -> Id { self.id }
}
