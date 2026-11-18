use std::path::PathBuf;
use crate::captures::list_images;
use crate::color::WHITE;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::document::{Document, pdf::PdfOpener};
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{CycleDir, Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::unit::scale_by_dpi;
use super::{Align, Bus, EntryId, Event, Hub, Id, ID_FEEDER, RenderData, RenderQueue,
            View, ViewId, SMALL_BAR_HEIGHT};
use super::button::Button;
use super::common::{add_pagination, toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use super::image::Image;
use super::file_actions::FileActions;
use super::label::Label;
use super::top_bar::TopBar;

pub struct Gallery {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    sketches: bool,
    files: Vec<PathBuf>,
    page: usize,
    selected: Option<usize>,
    error: Option<String>,
}

impl Gallery {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> Self {
        let mut gallery = Self {
            id: ID_FEEDER.next(), rect, children: Vec::new(), sketches: false,
            files: Vec::new(), page: 0, selected: None, error: None,
        };
        gallery.reload(context);
        gallery.rebuild(rq, context);
        gallery
    }

    fn folder(&self, context: &Context) -> PathBuf {
        context.library.home.join(if self.sketches {
            &context.settings.sketch.save_path
        } else {
            &context.settings.screenshot_path
        })
    }

    fn reload(&mut self, context: &Context) {
        let mut roots = vec![(self.folder(context), false)];
        if !self.sketches {
            roots.push((context.library.home.clone(), true));
            match std::env::current_dir() {
                Ok(path) => roots.push((path, true)),
                Err(e) => {
                    self.error = Some(format!("Can't find older screenshots: {}", e));
                    return;
                },
            }
        }
        match list_images(&roots) {
            Ok(files) => { self.files = files; self.error = None; },
            Err(e) => { self.files.clear(); self.error = Some(e.to_string()); },
        }
        self.page = self.page.min(self.pages().saturating_sub(1));
    }

    fn bar_height(&self) -> i32 {
        scale_by_dpi(SMALL_BAR_HEIGHT, CURRENT_DEVICE.dpi) as i32
    }

    fn per_page(&self) -> usize {
        ((self.rect.height() as i32 / self.bar_height()) - 4).max(1) as usize
    }

    fn pages(&self) -> usize {
        (self.files.len() + self.per_page() - 1) / self.per_page()
    }

    fn rebuild(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        self.children.clear();
        let rect = self.rect;
        let bar = self.bar_height();
        let gap = (bar / 12).max(1);
        let top = rect![rect.min.x, rect.min.y, rect.max.x, rect.min.y + bar];
        self.children.push(Box::new(TopBar::new(top, Event::Back, "Gallery".to_string(), context)));
        let footer = rect![rect.min.x, rect.max.y - bar, rect.max.x, rect.max.y];
        let third = rect.width() as i32 / 3;

        let (position, count) = if let Some(index) = self.selected {
            let path = &self.files[index];
            let label = rect![rect.min.x, top.max.y, rect.max.x, top.max.y + bar];
            self.children.push(Box::new(Label::new(label,
                path.file_name().unwrap_or_default().to_string_lossy().into_owned(), Align::Center)));
            let content = rect![rect.min.x + gap, label.max.y + gap,
                                rect.max.x - gap, footer.min.y - gap];
            let pixmap = PdfOpener::new().and_then(|opener| opener.open(path))
                .and_then(|mut doc| doc.preview_pixmap(content.width().saturating_sub(2) as f32,
                                                       content.height().saturating_sub(2) as f32, 1));
            if let Some(pixmap) = pixmap {
                self.children.push(Box::new(Image::new(content, pixmap)));
            } else {
                self.children.push(Box::new(Label::new(content,
                    "Couldn't open this image. Use Back to return.".to_string(), Align::Center)));
            }
            (index, self.files.len())
        } else {
            let tabs = rect![rect.min.x, top.max.y, rect.max.x, top.max.y + bar];
            for (i, (text, event, disabled)) in [
                ("Screenshots", EntryId::GalleryFolder(false), !self.sketches),
                ("Sketches", EntryId::GalleryFolder(true), self.sketches),
                ("Refresh", EntryId::Refresh, false),
            ].into_iter().enumerate() {
                let button_rect = rect![rect.min.x + i as i32 * third + gap, tabs.min.y + gap,
                                        rect.min.x + (i as i32 + 1) * third - gap, tabs.max.y - gap];
                self.children.push(Box::new(Button::new(button_rect, Event::Select(event), text.to_string())
                    .disabled(disabled)));
            }
            let folder_rect = rect![rect.min.x, tabs.max.y, rect.max.x, tabs.max.y + bar];
            self.children.push(Box::new(Label::new(folder_rect,
                self.folder(context).display().to_string(), Align::Center)));
            let start = self.page * self.per_page();
            for (row, path) in self.files.iter().skip(start).take(self.per_page()).enumerate() {
                let row_rect = rect![rect.min.x + gap, folder_rect.max.y + row as i32 * bar + gap,
                                     rect.max.x - gap, folder_rect.max.y + (row as i32 + 1) * bar - gap];
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                self.children.push(Box::new(Button::new(row_rect,
                    Event::Select(EntryId::GalleryOpen(start + row)), name)
                    .with_hold_event(move |point| Event::Select(EntryId::GalleryFile(start + row, point)))));
            }
            if self.files.is_empty() {
                let empty_rect = rect![rect.min.x, folder_rect.max.y, rect.max.x, footer.min.y];
                let message = self.error.clone().unwrap_or_else(|| {
                    if self.sketches { "No saved sketches yet." } else { "No screenshots yet." }.to_string()
                });
                self.children.push(Box::new(Label::new(empty_rect, message, Align::Center)));
            }
            (self.page, self.pages())
        };
        add_pagination(&mut self.children, footer, position, count);
        rq.add(RenderData::new(self.id, rect, UpdateMode::Full));
    }

    fn turn(&mut self, dir: CycleDir, rq: &mut RenderQueue, context: &mut Context) {
        let (position, count) = self.selected.map_or((self.page, self.pages()), |i| (i, self.files.len()));
        let next = match dir {
            CycleDir::Previous => position.saturating_sub(1),
            CycleDir::Next => (position + 1).min(count.saturating_sub(1)),
        };
        if next != position {
            if self.selected.is_some() { self.selected = Some(next); } else { self.page = next; }
            self.rebuild(rq, context);
        }
    }
}

impl View for Gallery {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::HoldFingerShort(point, _)) if self.selected.is_some() &&
                self.rect.includes(point) && point.y >= self.children[0].rect().max.y &&
                point.y < self.rect.max.y - self.bar_height() => {
                bus.push_back(Event::Select(EntryId::GalleryFile(self.selected.unwrap(), point)));
                true
            },
            Event::Select(EntryId::GalleryFile(index, point)) if index < self.files.len() => {
                let path = self.files[index].clone();
                let directory = if !self.sketches && (path.parent() == Some(context.library.home.as_path()) ||
                    std::env::current_dir().ok().as_deref() == path.parent()) { self.folder(context) }
                    else { path.parent().unwrap().to_path_buf() };
                self.children.push(Box::new(FileActions::new(self.rect,point,path,directory,rq,context)));true
            },
            Event::Close(ViewId::FileActions) => { FileActions::close(self, rq); true },
            Event::FileChanged(_, ref new_path) => {
                let was_selected = self.selected.is_some();
                self.selected = None;self.reload(context);
                if was_selected { self.selected = new_path.as_ref().and_then(|path|self.files.iter().position(|p|p==path)); }
                self.rebuild(rq,context);true
            },
            Event::Select(EntryId::GalleryFolder(sketches)) => {
                self.sketches = sketches;
                self.page = 0;
                self.selected = None;
                self.reload(context);
                self.rebuild(rq, context);
                true
            },
            Event::Select(EntryId::GalleryOpen(index)) if index < self.files.len() => {
                self.selected = Some(index);
                self.rebuild(rq, context);
                true
            },
            Event::Back if self.selected.is_some() => {
                self.page = self.selected.take().unwrap() / self.per_page();
                self.rebuild(rq, context);
                true
            },
            Event::Reseed | Event::Select(EntryId::Refresh) => {
                self.selected = None;
                self.reload(context);
                self.rebuild(rq, context);
                true
            },
            Event::Page(dir) => { self.turn(dir, rq, context); true },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::West, .. }) |
            Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Pressed, .. }) => {
                self.turn(CycleDir::Next, rq, context);
                true
            },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::East, .. }) |
            Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Pressed, .. }) => {
                self.turn(CycleDir::Previous, rq, context);
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, rq, context); true
            },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, rq, context); true
            },
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, rq, context); true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        fb.draw_rectangle(&rect, WHITE);
    }

    fn resize(&mut self, rect: Rectangle, _hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.rect = rect;
        self.page = self.page.min(self.pages().saturating_sub(1));
        self.rebuild(rq, context);
    }

    fn is_background(&self) -> bool { true }
    fn rect(&self) -> &Rectangle { &self.rect }
    fn rect_mut(&mut self) -> &mut Rectangle { &mut self.rect }
    fn children(&self) -> &Vec<Box<dyn View>> { &self.children }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> { &mut self.children }
    fn id(&self) -> Id { self.id }
}
