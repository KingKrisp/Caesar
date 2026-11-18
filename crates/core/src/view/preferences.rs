use std::path::Path;
use crate::color::WHITE;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{CycleDir, Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::settings::{SETTINGS_PATH, RotationLock};
use crate::settings::controls::{Action, Binding};
use crate::settings::preferences::{self, Preference, Value};
use crate::unit::scale_by_dpi;
use super::{Align, Bus, EntryId, Event, Hub, Id, ID_FEEDER, RenderData, RenderQueue, View, ViewId, SMALL_BAR_HEIGHT};
use super::button::Button;
use super::label::Label;
use super::notification::Notification;
use super::top_bar::TopBar;
use super::common::{add_pagination, toggle_main_menu, toggle_battery_menu, toggle_clock_menu};

const SECTIONS: &[&str] = &["Display and light", "Sleep, clock and USB", "Reading defaults", "Library",
                           "Reading controls", "Global shortcuts", "Device and storage"];

use Binding::*;
const CONTROL_GROUPS: &[(&str, &[Binding])] = &[
    ("Taps", &[TapNorthWest, TapNorth, TapNorthEast, TapWest, TapCenter, TapEast,
               TapSouthWest, TapSouth, TapSouthEast]),
    ("Swipes", &[SwipeWest, SwipeEast, SwipeNorth, SwipeSouth, SlantedNorthWest,
                 SlantedNorthEast, SlantedSouthWest, SlantedSouthEast]),
    ("Hold text", &[HoldShort, HoldLong]),
    ("Drawn gestures", &[ArrowWest, ArrowEast, ArrowNorth, ArrowSouth, CornerNorthWest,
                         CornerNorthEast, CornerSouthWest, CornerSouthEast, Cross, Diamond]),
    ("Two-finger swipes & arrows", &[MultiSwipeWest, MultiSwipeEast, MultiSwipeNorth, MultiSwipeSouth,
                                      MultiArrowWest, MultiArrowEast, MultiArrowNorth, MultiArrowSouth]),
    ("Two-finger corners & rotation", &[MultiCornerNorthWest, MultiCornerNorthEast,
                                          MultiCornerSouthWest, MultiCornerSouthEast, Rotate]),
    ("Pinch and spread", &[PinchHorizontal, PinchVertical, PinchDiagonal, SpreadHorizontal,
                           SpreadVertical, SpreadDiagonal]),
    ("Page buttons", &[Backward, Forward, HoldBackward, HoldForward]),
    ("Touch zones", &[]),
];

const ACTION_GROUPS: &[(&str, &[Action])] = &[
    ("Pages and chapters", &[Action::PreviousPage, Action::NextPage, Action::PreviousChapter,
                             Action::NextChapter, Action::FirstPage, Action::LastPage,
                             Action::GoToPage, Action::TableOfContents]),
    ("Bookmarks and annotations", &[Action::ToggleBookmark, Action::PreviousBookmark,
                                     Action::NextBookmark, Action::PreviousAnnotation, Action::NextAnnotation]),
    ("Reading and display", &[Action::ToggleBars, Action::Search, Action::ToggleFrontlight,
                              Action::ToggleInverted, Action::ToggleDithered, Action::Refresh]),
    ("Apps and shortcuts", &[Action::Settings, Action::Notes, Action::Gallery, Action::Screenshot, Action::Back]),
];

struct Row {
    title: String,
    event: Option<Event>,
    value: Option<String>,
}

impl Row {
    fn select(title: impl Into<String>, entry: EntryId) -> Self {
        Self { title: title.into(), event: Some(Event::Select(entry)), value: None }
    }
}

pub struct Preferences {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    section: Option<u8>,
    group: Option<usize>,
    choice: Option<Preference>,
    action_group: Option<usize>,
    page: usize,
    pages: usize,
    section_page: usize,
    return_page: usize,
    choice_page: usize,
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
            section: None, group: None, choice: None, action_group: None,
            page: 0, pages: 0, section_page: 0, return_page: 0, choice_page: 0 };
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
            Some(4) => match self.group {
                Some(i) if CONTROL_GROUPS[i].1.is_empty() => vec![StripWidth, CornerWidth],
                Some(i) => CONTROL_GROUPS[i].1.iter().copied().map(Bind).collect(),
                None => vec![ResetBindings],
            },
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
            Bind(binding) => {
                let actions: Vec<_> = if binding.is_global() {
                    Action::ALL.iter().copied().filter(|action| action.is_global()).collect()
                } else if let Some(group) = self.action_group {
                    ACTION_GROUPS[group].1.to_vec()
                } else {
                    vec![Action::Default, Action::Disabled]
                };
                actions.into_iter().map(|action| {
                    let label = if action == Action::Default {
                        format!("{} (default)", binding.default_label(&context.settings))
                    } else {
                        action.label().to_string()
                    };
                    (label, Value::Action(action))
                }).collect()
            },
            ResetBindings => texts(&["Restore"]),
        }
    }

    fn rows(&self, context: &mut Context) -> Vec<Row> {
        if let Some(preference) = self.choice {
            let current = preference.current(&context.settings);
            let mut rows: Vec<_> = self.options(preference, context).into_iter().enumerate()
                .map(|(index, (mut text, value))| {
                    if matches!(value, Value::Number(_)) {
                        text = preference.value_label(&value);
                    }
                    if value == current {
                        text = format!("✓ {}", text);
                    }
                    Row::select(text, EntryId::SettingsApply(index))
                }).collect();
            if matches!(preference, Preference::Bind(binding) if !binding.is_global()) && self.action_group.is_none() {
                if let Value::Action(action) = current {
                    if !matches!(action, Action::Default | Action::Disabled) {
                        rows.insert(0, Row { title: format!("Current action: {}", action.label()), event: None, value: None });
                    }
                }
                rows.extend(ACTION_GROUPS.iter().enumerate()
                    .map(|(i, (name, _))| Row::select(*name, EntryId::SettingsActions(i))));
            }
            return rows;
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
            return info.into_iter().map(|title| Row { title, event: None, value: None }).collect();
        }
        if self.section.is_none() {
            return SECTIONS.iter().enumerate()
                .map(|(i, name)| Row::select(*name, EntryId::SettingsSection(i as u8))).collect();
        }
        let mut rows = Vec::new();
        if self.section == Some(4) && self.group.is_none() {
            rows.extend(CONTROL_GROUPS.iter().enumerate()
                .filter(|(_, (_, bindings))| !bindings.contains(&Backward) || CURRENT_DEVICE.has_page_turn_buttons())
                .map(|(i, (name, _))| Row::select(*name, EntryId::SettingsGroup(i))));
        }
        rows.extend(self.items().into_iter().enumerate().map(|(index, preference)| {
            let mut row = Row::select(preference.label(), EntryId::SettingsChoose(index));
            if preference != Preference::ResetBindings {
                let current = preference.current(&context.settings);
                row.value = Some(match (preference, &current) {
                    (Preference::Bind(binding), Value::Action(Action::Default)) =>
                        binding.default_label(&context.settings).to_string(),
                    (Preference::Bind(_), _) => preference.value_label(&current),
                    _ => self.options(preference, context).into_iter().find(|(_, value)| *value == current)
                        .map_or_else(|| preference.value_label(&current), |(label, value)| {
                            if matches!(value, Value::Number(_)) { preference.value_label(&value) } else { label }
                        }),
                });
            }
            row
        }));
        rows
    }

    fn rebuild(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        self.children.clear();
        let rect = self.rect;
        let bar = scale_by_dpi(SMALL_BAR_HEIGHT, CURRENT_DEVICE.dpi) as i32;
        let row_height = bar * 4 / 5;
        let gap = (bar / 18).max(1);
        let title = if let Some(group) = self.action_group {
            ACTION_GROUPS[group].0
        } else if let Some(preference) = self.choice {
            preference.label()
        } else if let Some(group) = self.group {
            CONTROL_GROUPS[group].0
        } else {
            self.section.map_or("Settings", |s| SECTIONS[s as usize])
        };
        self.children.push(Box::new(TopBar::new(rect![rect.min.x, rect.min.y, rect.max.x, rect.min.y + bar],
            Event::Back, title.to_string(), context)));

        let mut y = rect.min.y + bar;
        let binding_title = [self.choice.map_or("", Preference::label)];
        let descriptions: &[&str] = if self.action_group.is_some() {
            &binding_title
        } else if let Some(preference) = self.choice {
            preference.help()
        } else {
            match (self.section, self.group) {
                (Some(2), _) => &["Saved book settings take priority over these defaults."],
                (Some(5), _) => &["Tap opposite corners with two fingers.",
                                  "Two-finger center tap always opens Settings."],
                _ => &[],
            }
        };
        for text in descriptions {
            self.children.push(Box::new(Label::new(rect![rect.min.x, y, rect.max.x, y + row_height],
                (*text).into(), Align::Center)));
            y += row_height;
        }

        let rows = self.rows(context);
        let available = rect.max.y - y;
        let per_page = if rows.len() <= (available / row_height).max(1) as usize {
            rows.len().max(1)
        } else {
            ((available - bar) / row_height).max(1) as usize
        };
        self.pages = rows.len().div_ceil(per_page);
        self.page = self.page.min(self.pages.saturating_sub(1));
        for row in rows.into_iter().skip(self.page * per_page).take(per_page) {
            let bounds = rect![rect.min.x + gap, y + gap, rect.max.x - gap, y + row_height - gap];
            self.children.push(match row.event {
                Some(event) => {
                    let mut button = Button::new(bounds, event, row.title);
                    if let Some(value) = row.value {
                        button = button.with_value(value);
                    }
                    Box::new(button) as Box<dyn View>
                },
                None => Box::new(Label::new(bounds, row.title, Align::Center)) as Box<dyn View>,
            });
            y += row_height;
        }

        add_pagination(&mut self.children,
            rect![rect.min.x, rect.max.y - bar, rect.max.x, rect.max.y], self.page, self.pages);
        rq.add(RenderData::new(self.id, rect, UpdateMode::Full));
    }

    fn choose(&mut self, index: usize, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let Some(preference) = self.choice else { return; };
        let Some((_, value)) = self.options(preference, context).into_iter().nth(index) else { return; };
        self.apply_value(preference, value, hub, rq, context);
    }

    fn apply_value(&mut self, preference: Preference, value: Value, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
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
        if self.choice.take().is_some() {
            self.action_group = None;
            self.page = self.return_page;
        }
        self.rebuild(rq, context);
    }

    fn turn(&mut self, dir: CycleDir, rq: &mut RenderQueue, context: &mut Context) {
        let page = match dir {
            CycleDir::Previous => self.page.saturating_sub(1),
            CycleDir::Next => (self.page + 1).min(self.pages.saturating_sub(1)),
        };
        if page != self.page {
            self.page = page;
            self.rebuild(rq, context);
        }
    }
}

impl View for Preferences {
    fn handle_event(&mut self, event: &Event, _hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *event {
            Event::Select(EntryId::SettingsSection(section)) if (section as usize) < SECTIONS.len() => {
                self.section = Some(section); self.group = None; self.choice = None; self.action_group = None;
                self.page = 0; self.rebuild(rq, context); true
            },
            Event::Select(EntryId::SettingsGroup(group)) if self.section == Some(4) &&
                self.group.is_none() && self.choice.is_none() &&
                CONTROL_GROUPS.get(group).is_some_and(|(_, bindings)|
                    !bindings.contains(&Backward) || CURRENT_DEVICE.has_page_turn_buttons()) => {
                self.group = Some(group); self.section_page = self.page; self.page = 0; self.rebuild(rq, context); true
            },
            Event::Select(EntryId::SettingsActions(group)) if self.action_group.is_none() &&
                matches!(self.choice, Some(Preference::Bind(binding)) if !binding.is_global()) &&
                group < ACTION_GROUPS.len() => {
                self.action_group = Some(group); self.choice_page = self.page; self.page = 0; self.rebuild(rq, context); true
            },
            Event::Select(EntryId::SettingsChoose(index)) if self.choice.is_none() => {
                if let Some(preference) = self.items().get(index).copied() {
                    let options = self.options(preference, context);
                    // Reading bindings show two quick actions alongside the full action categories.
                    if !matches!(preference, Preference::Bind(_)) && options.len() == 2 {
                        let current = preference.current(&context.settings);
                        let next = usize::from(options[0].1 == current);
                        let (_, value) = options.into_iter().nth(next).unwrap();
                        self.apply_value(preference, value, _hub, rq, context);
                    } else {
                        self.choice = Some(preference);
                        self.return_page = self.page;
                        self.page = 0;
                        self.rebuild(rq, context);
                    }
                }
                true
            },
            Event::Select(EntryId::SettingsApply(index)) => { self.choose(index, _hub, rq, context); true },
            Event::Back if self.action_group.is_some() => {
                self.action_group = None; self.page = self.choice_page; self.rebuild(rq, context); true
            },
            Event::Back if self.choice.is_some() => {
                self.choice = None; self.page = self.return_page; self.rebuild(rq, context); true
            },
            Event::Back if self.group.is_some() => {
                self.group = None; self.page = self.section_page; self.rebuild(rq, context); true
            },
            Event::Back if self.section.is_some() => { self.section = None; self.page = 0; self.rebuild(rq, context); true },
            Event::Reseed => { self.rebuild(rq, context); true },
            Event::Page(dir) => { self.turn(dir, rq, context); true },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Pressed, .. }) |
            Event::Gesture(GestureEvent::Swipe { dir: Dir::West, .. }) => { self.turn(CycleDir::Next, rq, context); true },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Pressed, .. }) |
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_keep_every_reading_binding_and_action_available_once() {
        let mut bindings: Vec<_> = CONTROL_GROUPS.iter().flat_map(|(_, bindings)| bindings.iter().copied()).collect();
        bindings.sort_unstable();
        assert_eq!(bindings, Binding::ALL.iter().copied().filter(|b| !b.is_global()).collect::<Vec<_>>());

        let mut actions: Vec<_> = ACTION_GROUPS.iter().flat_map(|(_, actions)| actions.iter().copied()).collect();
        actions.extend([Action::Default, Action::Disabled]);
        actions.sort_unstable();
        assert_eq!(actions, Action::ALL);
    }
}
