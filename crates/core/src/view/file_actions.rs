use super::{
    button::Button, input_field::InputField, keyboard::Keyboard, label::Label, top_bar::TopBar,
};
use super::{
    Align, Bus, EntryId, Event, Hub, Id, RenderData, RenderQueue, View, ViewId, ID_FEEDER,
    SMALL_BAR_HEIGHT,
};
use crate::{
    color::WHITE,
    context::Context,
    device::CURRENT_DEVICE,
    font::Fonts,
    framebuffer::{Framebuffer, UpdateMode},
    geom::Rectangle,
    unit::scale_by_dpi,
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Actions,
    Rename,
    Delete,
}

/// A modal file sheet. Delete requires a separate confirmation screen.
pub struct FileActions {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    path: PathBuf,
    directory: PathBuf,
    mode: Mode,
    error: Option<String>,
}
impl FileActions {
    pub fn new(
        rect: Rectangle,
        path: PathBuf,
        directory: PathBuf,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> Self {
        let mut view = Self {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            path,
            directory,
            mode: Mode::Actions,
            error: None,
        };
        view.rebuild(None, rq, context);
        view
    }
    fn rebuild(&mut self, text: Option<String>, rq: &mut RenderQueue, context: &mut Context) {
        self.children.clear();
        let r = self.rect;
        let bar = scale_by_dpi(SMALL_BAR_HEIGHT, CURRENT_DEVICE.dpi) as i32;
        let gap = (bar / 10).max(1);
        self.children.push(Box::new(TopBar::new(
            rect![r.min.x, r.min.y, r.max.x, r.min.y + bar],
            Event::Back,
            match self.mode {
                Mode::Rename => "Rename file",
                Mode::Delete => "Delete file",
                _ => "File",
            }
            .into(),
            context,
        )));
        let caption = self.error.clone().unwrap_or_else(|| {
            if self.mode == Mode::Rename {
                format!(
                    "New name (.{} stays unchanged)",
                    self.path.extension().unwrap_or_default().to_string_lossy()
                )
            } else {
                self.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            }
        });
        self.children.push(Box::new(Label::new(
            rect![r.min.x, r.min.y + bar, r.max.x, r.min.y + 2 * bar],
            caption,
            Align::Center,
        )));
        match self.mode {
            Mode::Rename => {
                let text = text.unwrap_or_else(|| {
                    self.path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                });
                let field = InputField::new(
                    rect![
                        r.min.x + gap,
                        r.min.y + 2 * bar,
                        r.max.x - gap,
                        r.min.y + 3 * bar
                    ],
                    ViewId::FileNameInput,
                )
                .text(&text, context)
                .focused();
                self.children.push(Box::new(field));
                self.children.push(Box::new(Button::new(
                    rect![
                        r.min.x + gap,
                        r.min.y + 3 * bar + gap,
                        r.min.x + r.width() as i32 / 2 - gap,
                        r.min.y + 4 * bar - gap
                    ],
                    Event::Select(EntryId::FileRenameSave),
                    "Save name".into(),
                )));
                self.children.push(Box::new(Button::new(
                    rect![
                        r.min.x + r.width() as i32 / 2 + gap,
                        r.min.y + 3 * bar + gap,
                        r.max.x - gap,
                        r.min.y + 4 * bar - gap
                    ],
                    Event::Back,
                    "Cancel".into(),
                )));
                let mut kb = rect![r.min.x, r.max.y - 4 * bar, r.max.x, r.max.y];
                self.children
                    .push(Box::new(Keyboard::new(&mut kb, false, context)));
            }
            Mode::Delete => {
                self.children.push(Box::new(Label::new(
                    rect![r.min.x, r.min.y + 2 * bar, r.max.x, r.min.y + 3 * bar],
                    "Permanently delete this file?".into(),
                    Align::Center,
                )));
                self.children.push(Box::new(Button::new(
                    rect![
                        r.min.x + gap,
                        r.min.y + 3 * bar + gap,
                        r.max.x - gap,
                        r.min.y + 4 * bar - gap
                    ],
                    Event::Select(EntryId::FileDeleteConfirm),
                    "Delete file".into(),
                )));
            }
            Mode::Actions => {
                for (row, label, event) in [
                    (2, "Rename", EntryId::FileRename),
                    (3, "Delete", EntryId::FileDelete),
                ] {
                    self.children.push(Box::new(Button::new(
                        rect![
                            r.min.x + gap,
                            r.min.y + row * bar + gap,
                            r.max.x - gap,
                            r.min.y + (row + 1) * bar - gap
                        ],
                        Event::Select(event),
                        label.into(),
                    )));
                }
            }
        }
        if self.mode != Mode::Rename {
            self.children.push(Box::new(Button::new(
                rect![
                    r.min.x + gap,
                    r.max.y - bar + gap,
                    r.max.x - gap,
                    r.max.y - gap
                ],
                Event::Back,
                "Cancel".into(),
            )));
        }
        rq.add(RenderData::new(self.id, r, UpdateMode::Full));
    }
    fn rename(&mut self, name: &str, bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) {
        match crate::file_ops::rename(&self.path, &self.directory, name) {
            Ok(path) => {
                context.kb_rect = Rectangle::default();
                bus.push_back(Event::FileChanged(self.path.clone(), Some(path)));
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.rebuild(Some(name.into()), rq, context);
            }
        }
    }
}
impl View for FileActions {
    fn handle_event(
        &mut self,
        event: &Event,
        hub: &Hub,
        bus: &mut Bus,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> bool {
        match event {
            Event::Select(EntryId::FileRename) => {
                self.mode = Mode::Rename;
                self.error = None;
                self.rebuild(None, rq, context);
                hub.send(Event::Focus(Some(ViewId::FileNameInput))).ok();
                true
            }
            Event::Select(EntryId::FileDelete) => {
                self.mode = Mode::Delete;
                self.error = None;
                self.rebuild(None, rq, context);
                true
            }
            Event::Select(EntryId::FileRenameSave) => {
                let text = self.children.iter().find_map(|v| {
                    v.downcast_ref::<InputField>()
                        .map(|f| f.value().to_string())
                });
                if let Some(text) = text {
                    self.rename(&text, bus, rq, context);
                }
                true
            }
            Event::Submit(ViewId::FileNameInput, text) => {
                self.rename(text, bus, rq, context);
                true
            }
            Event::Select(EntryId::FileDeleteConfirm) if self.mode == Mode::Delete => {
                match crate::file_ops::delete(&self.path) {
                    Ok(()) => bus.push_back(Event::FileChanged(self.path.clone(), None)),
                    Err(e) => {
                        self.error = Some(e.to_string());
                        self.rebuild(None, rq, context);
                    }
                }
                true
            }
            Event::Back => {
                context.kb_rect = Rectangle::default();
                bus.push_back(Event::Close(ViewId::FileActions));
                true
            }
            Event::Focus(_) | Event::Show(ViewId::Keyboard) => true,
            Event::Gesture(_)
            | Event::Device(crate::input::DeviceEvent::Finger { .. })
            | Event::Page(_) => true,
            _ => false,
        }
    }
    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        fb.draw_rectangle(&rect, WHITE);
    }
    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let text = self.children.iter().find_map(|v| {
            v.downcast_ref::<InputField>()
                .map(|f| f.value().to_string())
        });
        self.rect = rect;
        self.rebuild(text, rq, context);
        if self.mode == Mode::Rename {
            hub.send(Event::Focus(Some(ViewId::FileNameInput))).ok();
        }
    }
    fn is_background(&self) -> bool {
        true
    }
    fn view_id(&self) -> Option<ViewId> {
        Some(ViewId::FileActions)
    }
    fn rect(&self) -> &Rectangle {
        &self.rect
    }
    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }
    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }
    fn id(&self) -> Id {
        self.id
    }
}
