mod buffer;
mod editor;

use std::path::PathBuf;
use crate::color::WHITE;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{CycleDir, Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::notes::{NoteFile, NoteSummary, list_notes, preview_html};
use crate::unit::scale_by_dpi;
use super::{Align, Bus, EntryId, Event, Hub, Id, ID_FEEDER, RenderData, RenderQueue,
            View, ViewId, SMALL_BAR_HEIGHT};
use super::button::Button;
use super::common::{locate, toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use super::keyboard::Keyboard;
use super::label::Label;
use super::notification::Notification;
use super::top_bar::TopBar;
use buffer::Buffer;
use editor::Editor;

pub struct Notes {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    folder: PathBuf,
    notes: Vec<NoteSummary>,
    page: usize,
    document: Option<NoteFile>,
    status_index: Option<usize>,
    error: Option<String>,
}

impl Notes {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> Self {
        let mut notes = Self {
            id: ID_FEEDER.next(), rect, children: Vec::new(),
            folder: context.library.home.join(&context.settings.notes_path),
            notes: Vec::new(), page: 0, document: None, status_index: None, error: None,
        };
        notes.reload();
        notes.rebuild(None, rq, context);
        notes
    }

    fn editor(&self) -> Option<&Editor> {
        locate::<Editor>(self).and_then(|i| self.children[i].downcast_ref::<Editor>())
    }

    fn editor_mut(&mut self) -> Option<&mut Editor> {
        let index = locate::<Editor>(self)?;
        self.children[index].downcast_mut::<Editor>()
    }

    fn reload(&mut self) {
        match list_notes(&self.folder) {
            Ok(notes) => { self.notes = notes; self.error = None; },
            Err(e) => { self.notes.clear(); self.error = Some(format!("Can't list notes: {}", e)); },
        }
        self.page = self.page.min(self.pages().saturating_sub(1));
    }

    fn bar(&self) -> i32 { scale_by_dpi(SMALL_BAR_HEIGHT, CURRENT_DEVICE.dpi) as i32 }
    fn per_page(&self) -> usize { (self.rect.height() as i32 / self.bar() - 4).max(1) as usize }
    fn pages(&self) -> usize { (self.notes.len() + self.per_page() - 1) / self.per_page() }

    fn status(&mut self, text: &str, rq: &mut RenderQueue) {
        if let Some(index) = self.status_index {
            self.children[index].downcast_mut::<Label>().unwrap().update(text, rq);
        }
    }

    fn changed(&mut self, rq: &mut RenderQueue) {
        let dirty = self.editor().zip(self.document.as_ref())
            .map_or(false, |(editor, note)| editor.buffer.text != note.saved_text);
        self.status(if dirty { "Unsaved changes" } else { "Saved" }, rq);
    }

    fn save(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> bool {
        let Some(text) = self.editor().map(|e| e.buffer.text.clone()) else { return true; };
        let Some(document) = self.document.as_mut() else { return true; };
        match document.save(&self.folder, &text) {
            Ok(()) => { self.changed(rq); true },
            Err(e) => {
                self.status("Save failed — edits kept open", rq);
                self.children.push(Box::new(Notification::new(format!("Couldn't save note: {}", e), hub, rq, context)));
                false
            },
        }
    }

    fn button(&mut self, rect: Rectangle, event: Event, text: &str, disabled: bool) {
        self.children.push(Box::new(Button::new(rect, event, text.to_string()).disabled(disabled)));
    }

    fn rebuild(&mut self, buffer: Option<Buffer>, rq: &mut RenderQueue, context: &mut Context) {
        self.children.clear();
        self.status_index = None;
        let rect = self.rect;
        let bar = self.bar();
        let gap = (bar / 12).max(1);
        self.children.push(Box::new(TopBar::new(
            rect![rect.min.x, rect.min.y, rect.max.x, rect.min.y + bar],
            Event::Back, "Notes".to_string(), context)));
        if self.document.is_some() {
            let width = rect.width() as i32 / 5;
            for (i, (label, action)) in [("Heading", EntryId::NoteHeading), ("Bold", EntryId::NoteBold),
                                        ("Italic", EntryId::NoteItalic), ("Preview", EntryId::NotePreview),
                                        ("Save", EntryId::Save)].into_iter().enumerate() {
                self.button(rect![rect.min.x + i as i32 * width + gap, rect.min.y + bar + gap,
                                  rect.min.x + (i as i32 + 1) * width - gap, rect.min.y + 2 * bar - gap],
                            Event::Select(action), label, false);
            }
            let mut kb_rect = rect![rect.min.x, rect.max.y - 4 * bar, rect.max.x, rect.max.y];
            let keyboard = Keyboard::new(&mut kb_rect, false, context);
            let status = rect![rect.min.x, kb_rect.min.y - bar, rect.max.x, kb_rect.min.y];
            let editor_rect = rect![rect.min.x + gap, rect.min.y + 2 * bar + gap,
                                    rect.max.x - gap, status.min.y - gap];
            let buffer = buffer.unwrap_or_else(|| Buffer::new(self.document.as_ref().unwrap().saved_text.clone()));
            self.children.push(Box::new(Editor::new(editor_rect, buffer, &mut context.fonts)));
            let side = rect.width() as i32 / 5;
            self.button(rect![status.min.x + gap, status.min.y + gap, status.min.x + side - gap, status.max.y - gap],
                        Event::Page(CycleDir::Previous), "Up", false);
            self.button(rect![status.max.x - side + gap, status.min.y + gap, status.max.x - gap, status.max.y - gap],
                        Event::Page(CycleDir::Next), "Down", false);
            self.status_index = Some(self.children.len());
            self.children.push(Box::new(Label::new(
                rect![status.min.x + side, status.min.y, status.max.x - side, status.max.y],
                "Saved".to_string(), Align::Center)));
            self.children.push(Box::new(keyboard));
            self.changed(rq);
        } else {
            let third = rect.width() as i32 / 3;
            self.button(rect![rect.min.x + gap, rect.min.y + bar + gap,
                              rect.min.x + third - gap, rect.min.y + 2 * bar - gap],
                        Event::Select(EntryId::Refresh), "Refresh", false);
            self.button(rect![rect.max.x - third + gap, rect.min.y + bar + gap,
                              rect.max.x - gap, rect.min.y + 2 * bar - gap],
                        Event::Select(EntryId::NoteNew), "+ New Note", false);
            self.children.push(Box::new(Label::new(
                rect![rect.min.x, rect.min.y + 2 * bar, rect.max.x, rect.min.y + 3 * bar],
                self.folder.display().to_string(), Align::Center)));
            let start = self.page * self.per_page();
            let entries: Vec<_> = self.notes.iter().skip(start).take(self.per_page())
                .map(|note| note.title.clone()).collect();
            for (row, title) in entries.into_iter().enumerate() {
                let y = rect.min.y + 3 * bar + row as i32 * bar;
                self.button(rect![rect.min.x + gap, y + gap, rect.max.x - gap, y + bar - gap],
                            Event::Select(EntryId::NoteOpen(start + row)), &title, false);
            }
            if self.notes.is_empty() {
                self.children.push(Box::new(Label::new(
                    rect![rect.min.x, rect.min.y + 3 * bar, rect.max.x, rect.max.y - bar],
                    self.error.clone().unwrap_or_else(|| "No notes yet. Tap + New Note to begin.".to_string()), Align::Center)));
            }
            for (text, dir, x, disabled) in [
                ("Previous", CycleDir::Previous, rect.min.x, self.page == 0),
                ("Next", CycleDir::Next, rect.max.x - third, self.page + 1 >= self.pages()),
            ] {
                self.button(rect![x + gap, rect.max.y - bar + gap, x + third - gap, rect.max.y - gap],
                            Event::Page(dir), text, disabled);
            }
            let count = self.pages();
            self.children.push(Box::new(Label::new(
                rect![rect.min.x + third, rect.max.y - bar, rect.max.x - third, rect.max.y],
                format!("{} / {}", if count == 0 { 0 } else { self.page + 1 }, count), Align::Center)));
        }
        rq.add(RenderData::new(self.id, rect, UpdateMode::Full));
    }

    fn turn(&mut self, dir: CycleDir, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(editor) = self.editor_mut() {
            editor.scroll(dir, rq, &mut context.fonts);
        } else {
            self.page = match dir {
                CycleDir::Previous => self.page.saturating_sub(1),
                CycleDir::Next => (self.page + 1).min(self.pages().saturating_sub(1)),
            };
            self.rebuild(None, rq, context);
        }
    }
}

impl View for Notes {
    fn handle_event(&mut self, event: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *event {
            Event::Select(EntryId::NoteNew) if self.document.is_none() => {
                self.document = Some(NoteFile::default());
                self.rebuild(Some(Buffer::default()), rq, context);
                true
            },
            Event::Select(EntryId::NoteOpen(index)) if self.document.is_none() && index < self.notes.len() => {
                match NoteFile::load(&self.notes[index].path) {
                    Ok(note) => {
                        let mut buffer = Buffer::new(note.saved_text.clone());
                        buffer.cursor = 0;
                        self.document = Some(note);
                        self.rebuild(Some(buffer), rq, context);
                    },
                    Err(e) => self.children.push(Box::new(Notification::new(format!("Couldn't open note: {}", e), hub, rq, context))),
                }
                true
            },
            Event::Select(EntryId::NoteChanged) => { self.changed(rq); true },
            Event::Select(ref action @ (EntryId::NoteHeading | EntryId::NoteBold | EntryId::NoteItalic)) => {
                if let Some(editor) = self.editor_mut() {
                    match action {
                        EntryId::NoteHeading => editor.buffer.heading(),
                        EntryId::NoteBold => editor.buffer.emphasis("**"),
                        _ => editor.buffer.emphasis("*"),
                    }
                    editor.refresh(rq, &mut context.fonts);
                    self.changed(rq);
                }
                true
            },
            Event::Select(EntryId::Save) => { self.save(hub, rq, context); true },
            Event::Save => self.save(hub, rq, context),
            Event::Select(EntryId::NotePreview) => {
                if self.save(hub, rq, context) {
                    if let Some(editor) = self.editor() {
                        let html = format!("<html><head><title>Note preview</title></head><body>{}</body></html>",
                                           preview_html(&editor.buffer.text));
                        hub.send(Event::OpenHtml(html, None)).ok();
                        hub.send(Event::Toggle(ViewId::TopBottomBars)).ok();
                    }
                }
                true
            },
            Event::Back if self.document.is_some() => {
                if self.save(hub, rq, context) {
                    self.document = None;
                    self.reload();
                    self.rebuild(None, rq, context);
                }
                true
            },
            Event::Suspend => { self.save(hub, rq, context); false },
            Event::Select(EntryId::Quit | EntryId::Reboot) if self.document.is_some() => !self.save(hub, rq, context),
            Event::Select(EntryId::Launch(_)) if self.document.is_some() => !self.save(hub, rq, context),
            Event::Reseed => {
                if self.document.is_some() {
                    if let Some(top) = self.children[0].downcast_mut::<TopBar>() { top.reseed(rq, context); }
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
                } else { self.reload(); self.rebuild(None, rq, context); }
                true
            },
            Event::Select(EntryId::Refresh) if self.document.is_none() => {
                self.reload(); self.rebuild(None, rq, context); true
            },
            Event::Page(dir) => { self.turn(dir, rq, context); true },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::West | Dir::North, .. }) |
            Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Pressed, .. }) => {
                self.turn(CycleDir::Next, rq, context); true
            },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::East | Dir::South, .. }) |
            Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Pressed, .. }) => {
                self.turn(CycleDir::Previous, rq, context); true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => { toggle_main_menu(self, rect, None, rq, context); true },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => { toggle_battery_menu(self, rect, None, rq, context); true },
            Event::ToggleNear(ViewId::ClockMenu, rect) => { toggle_clock_menu(self, rect, None, rq, context); true },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) { fb.draw_rectangle(&rect, WHITE); }
    fn resize(&mut self, rect: Rectangle, _hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let buffer = self.editor().map(|e| e.buffer.clone());
        self.rect = rect;
        self.page = self.page.min(self.pages().saturating_sub(1));
        self.rebuild(buffer, rq, context);
    }
    fn is_background(&self) -> bool { true }
    fn rect(&self) -> &Rectangle { &self.rect }
    fn rect_mut(&mut self) -> &mut Rectangle { &mut self.rect }
    fn children(&self) -> &Vec<Box<dyn View>> { &self.children }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> { &mut self.children }
    fn id(&self) -> Id { self.id }
}
