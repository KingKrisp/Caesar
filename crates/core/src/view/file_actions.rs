use super::{
    button::Button,
    common::{locate_by_id, overlapping_rectangle},
    input_field::InputField,
    keyboard::Keyboard,
    label::Label,
    menu::{Menu, MenuKind},
    Align, Bus, EntryId, EntryKind, Event, Hub, Id, RenderData, RenderQueue, View, ViewId,
    BORDER_RADIUS_MEDIUM, ID_FEEDER, SMALL_BAR_HEIGHT, THICKNESS_LARGE,
};
use crate::{
    color::{BLACK, WHITE},
    context::Context,
    device::CURRENT_DEVICE,
    font::Fonts,
    framebuffer::{Framebuffer, UpdateMode},
    geom::{BorderSpec, CornerSpec, Point, Rectangle},
    gesture::GestureEvent,
    unit::scale_by_dpi,
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Actions,
    Rename,
    Delete,
}

pub struct FileActions {
    id: Id,
    rect: Rectangle,
    bounds: Rectangle,
    anchor: Point,
    previous_keyboard: Rectangle,
    children: Vec<Box<dyn View>>,
    path: PathBuf,
    directory: PathBuf,
    mode: Mode,
    error: Option<String>,
}

impl FileActions {
    pub fn new(
        bounds: Rectangle,
        anchor: Point,
        path: PathBuf,
        directory: PathBuf,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> Self {
        let mut view = Self {
            id: ID_FEEDER.next(),
            rect: Rectangle::default(),
            bounds,
            anchor,
            previous_keyboard: context.kb_rect,
            children: Vec::new(),
            path,
            directory,
            mode: Mode::Actions,
            error: None,
        };
        view.rebuild(None, rq, context);
        view
    }

    pub fn close(view: &mut dyn View, rq: &mut RenderQueue) {
        if let Some(index) = locate_by_id(view, ViewId::FileActions) {
            let popup = view.children_mut().remove(index);
            rq.add(RenderData::expose(
                overlapping_rectangle(popup.as_ref()),
                UpdateMode::Gui,
            ));
        }
    }

    fn rebuild(&mut self, text: Option<String>, rq: &mut RenderQueue, context: &mut Context) {
        let old = overlapping_rectangle(self);
        self.children.clear();

        if self.mode == Mode::Actions {
            let menu = Menu::new(
                Rectangle::from_point(self.anchor),
                ViewId::FileActions,
                MenuKind::Contextual,
                vec![
                    EntryKind::Command("Rename".into(), EntryId::FileRename),
                    EntryKind::Command("Delete".into(), EntryId::FileDelete),
                ],
                context,
            )
            .root(false);
            self.rect = *menu.rect();
            if old.width() > 0 && old != self.rect {
                rq.add(RenderData::expose(old, UpdateMode::Gui));
            }
            rq.add(RenderData::new(menu.id(), self.rect, UpdateMode::Gui));
            self.children.push(Box::new(menu));
            return;
        }

        let bar = scale_by_dpi(SMALL_BAR_HEIGHT, CURRENT_DEVICE.dpi) as i32;
        let gap = (bar / 10).max(1);
        let bounds = self.bounds;
        let keyboard = if self.mode == Mode::Rename {
            let mut kb = rect![
                bounds.min.x,
                bounds.max.y - 4 * bar,
                bounds.max.x,
                bounds.max.y
            ];
            Some(Keyboard::new(&mut kb, false, context))
        } else {
            None
        };
        let bottom = keyboard.as_ref().map_or(bounds.max.y, |kb| kb.rect().min.y);
        let height = 3 * bar + 2 * gap;
        let width = (6 * bar).min(bounds.width() as i32 - 2 * gap);
        let x = (self.anchor.x - width / 2).clamp(bounds.min.x + gap, bounds.max.x - gap - width);
        let top = bounds.min.y + gap;
        let preferred_y = if bottom - self.anchor.y >= self.anchor.y - top {
            self.anchor.y + gap
        } else {
            self.anchor.y - height - gap
        };
        let y = preferred_y.clamp(top, (bottom - height - gap).max(top));
        self.rect = rect![x, y, x + width, y + height];

        let panel = self.rect;
        let (left, right) = (panel.min.x + gap, panel.max.x - gap);
        let top = panel.min.y + gap;
        let (caption, label, event) = if self.mode == Mode::Rename {
            let extension = self.path.extension().unwrap_or_default().to_string_lossy();
            (
                format!("Rename file (.{} stays unchanged)", extension),
                "Save name",
                EntryId::FileRenameSave,
            )
        } else {
            (
                "Permanently delete this file?".into(),
                "Delete file",
                EntryId::FileDeleteConfirm,
            )
        };
        self.children.push(Box::new(Label::new(
            rect![left, top, right, top + bar],
            self.error.clone().unwrap_or(caption),
            Align::Center,
        )));

        let input_rect = rect![left, top + bar, right, top + 2 * bar];
        if self.mode == Mode::Rename {
            let text = text.unwrap_or_else(|| {
                self.path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            let field = InputField::new(input_rect, ViewId::FileNameInput)
                .text(&text, context)
                .focused();
            self.children.push(Box::new(field));
        } else {
            self.children.push(Box::new(Label::new(
                input_rect,
                self.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                Align::Center,
            )));
        }

        let mid = panel.center().x;
        let y = top + 2 * bar;
        self.children.push(Box::new(Button::new(
            rect![left, y + gap, mid - gap, y + bar],
            Event::Back,
            "Cancel".into(),
        )));
        self.children.push(Box::new(Button::new(
            rect![mid + gap, y + gap, right, y + bar],
            Event::Select(event),
            label.into(),
        )));
        if let Some(keyboard) = keyboard {
            rq.add(RenderData::new(
                keyboard.id(),
                *keyboard.rect(),
                UpdateMode::Gui,
            ));
            self.children.push(Box::new(keyboard));
        }
        if old.width() > 0 && old != overlapping_rectangle(self) {
            rq.add(RenderData::expose(old, UpdateMode::Gui));
        }
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    fn rename(&mut self, name: &str, bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) {
        match crate::file_ops::rename(&self.path, &self.directory, name) {
            Ok(path) => {
                context.kb_rect = self.previous_keyboard;
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
                context.kb_rect = self.previous_keyboard;
                bus.push_back(Event::Close(ViewId::FileActions));
                true
            }
            Event::Gesture(GestureEvent::Tap(point))
                if !self.rect.includes(*point)
                    && (self.mode != Mode::Rename || !context.kb_rect.includes(*point)) =>
            {
                context.kb_rect = self.previous_keyboard;
                bus.push_back(Event::Close(ViewId::FileActions));
                true
            }
            Event::CloseSub(ViewId::FileActions)
            | Event::Validate
            | Event::Focus(_)
            | Event::Show(ViewId::Keyboard) => true,
            Event::Gesture(_)
            | Event::Device(crate::input::DeviceEvent::Finger { .. })
            | Event::Page(_) => true,
            _ => false,
        }
    }
    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        if self.mode == Mode::Actions {
            return;
        }
        let dpi = CURRENT_DEVICE.dpi;
        let radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;
        fb.draw_rounded_rectangle_with_border(
            &self.rect,
            &CornerSpec::Uniform(radius),
            &BorderSpec {
                thickness,
                color: BLACK,
            },
            &WHITE,
        );
    }
    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let text = self.children.iter().find_map(|v| {
            v.downcast_ref::<InputField>()
                .map(|f| f.value().to_string())
        });
        self.bounds = rect;
        self.rebuild(text, rq, context);
        if self.mode == Mode::Rename {
            hub.send(Event::Focus(Some(ViewId::FileNameInput))).ok();
        }
    }
    fn is_background(&self) -> bool {
        self.mode != Mode::Actions
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
