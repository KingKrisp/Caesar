use crate::device::CURRENT_DEVICE;
use crate::geom::{Point, Rectangle, CornerSpec, BorderSpec};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use super::{THICKNESS_MEDIUM, BORDER_RADIUS_LARGE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::input::{DeviceEvent, FingerStatus};
use crate::gesture::GestureEvent;
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::unit::scale_by_dpi;
use crate::context::Context;

pub struct Button {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    event: Event,
    hold_event: Option<Box<dyn Fn(Point) -> Event>>,
    text: String,
    value: Option<String>,
    active: bool,
    pub disabled: bool,
}

impl Button {
    pub fn new(rect: Rectangle, event: Event, text: String) -> Button {
        Button {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            event,
            hold_event: None,
            text,
            value: None,
            active: false,
            disabled: false,
        }
    }

    pub fn with_value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    pub fn with_hold_event(mut self, event: impl Fn(Point) -> Event + 'static) -> Self {
        self.hold_event = Some(Box::new(event));
        self
    }

    pub fn disabled(mut self, value: bool) -> Button {
        self.disabled = value;
        self
    }
}

impl View for Button {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, position, .. }) if !self.disabled => {
                match status {
                    FingerStatus::Down if self.rect.includes(position) => {
                        self.active = true;
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Fast));
                        true
                    },
                    FingerStatus::Up if self.active => {
                        self.active = false;
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    _ => false,
                }
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                if !self.disabled {
                    bus.push_back(self.event.clone());
                }
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, _))
                if !self.disabled && self.rect.includes(center) => {
                if let Some(event) = &self.hold_event {
                    self.active = false;
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                    bus.push_back(event(center));
                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };
        let foreground = if self.disabled { scheme[2] } else { scheme[1] };

        let border_radius = scale_by_dpi(BORDER_RADIUS_LARGE, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness,
                                                            color: foreground },
                                              &scheme[0]);

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let width = self.rect.width() as i32;
        let y = self.rect.max.y - (self.rect.height() as i32 - x_height) / 2;

        if let Some(value) = &self.value {
            let split = width / 2;
            let title = font.plan(&self.text, Some(split - padding * 2), None);
            let value = font.plan(value, Some(width - split - padding), None);
            font.render(fb, foreground, &title, pt!(self.rect.min.x + padding, y));
            font.render(fb, foreground, &value, pt!(self.rect.max.x - padding - value.width, y));
        } else {
            let plan = font.plan(&self.text, Some(width - padding), None);
            let x = self.rect.min.x + (width - plan.width) / 2;
            font.render(fb, foreground, &plan, pt!(x, y));
        }
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
