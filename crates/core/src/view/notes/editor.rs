use std::ops::Range;
use crate::color::{BLACK, WHITE};
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{CycleDir, Rectangle};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::view::{Bus, EntryId, Event, Hub, Id, ID_FEEDER, RenderData, RenderQueue, View};
use super::buffer::{Buffer, wrap_lines};

pub(super) struct Editor {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pub buffer: Buffer,
    first_line: usize,
}

impl Editor {
    pub fn new(rect: Rectangle, buffer: Buffer, fonts: &mut Fonts) -> Self {
        let mut editor = Self { id: ID_FEEDER.next(), rect, buffer, first_line: 0, children: Vec::new() };
        editor.reveal_cursor(fonts);
        editor
    }

    fn layout(&self, fonts: &mut Fonts) -> (i32, i32, usize, Vec<Range<usize>>) {
        let font = font_from_style(fonts, &NORMAL_STYLE, CURRENT_DEVICE.dpi);
        let padding = font.em() as i32 / 2;
        let height = font.line_height().max(font.x_heights.0 as i32 * 3);
        let width = (self.rect.width() as i32 - 2 * padding).max(1);
        let rows = ((self.rect.height() as i32 - 2 * padding) / height).max(1) as usize;
        let lines = wrap_lines(&self.buffer.text, width, |s| font.plan(s, None, None).width);
        (padding, height, rows, lines)
    }

    pub fn reveal_cursor(&mut self, fonts: &mut Fonts) {
        let (_, _, rows, lines) = self.layout(fonts);
        let line = lines.iter().rposition(|r| r.start <= self.buffer.cursor).unwrap_or(0);
        if line < self.first_line { self.first_line = line; }
        if line >= self.first_line + rows { self.first_line = line + 1 - rows; }
        self.first_line = self.first_line.min(lines.len().saturating_sub(rows));
    }

    pub fn scroll(&mut self, dir: CycleDir, rq: &mut RenderQueue, fonts: &mut Fonts) {
        let (_, _, rows, lines) = self.layout(fonts);
        self.first_line = match dir {
            CycleDir::Previous => self.first_line.saturating_sub(rows),
            CycleDir::Next => (self.first_line + rows).min(lines.len().saturating_sub(rows)),
        };
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    pub fn refresh(&mut self, rq: &mut RenderQueue, fonts: &mut Fonts) {
        self.reveal_cursor(fonts);
        rq.add(RenderData::no_wait(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Editor {
    fn handle_event(&mut self, event: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *event {
            Event::Keyboard(event) => {
                self.buffer.keyboard(event);
                self.refresh(rq, &mut context.fonts);
                bus.push_back(Event::Select(EntryId::NoteChanged));
                true
            },
            Event::Gesture(GestureEvent::Tap(point)) if self.rect.includes(point) => {
                let (padding, height, rows, lines) = self.layout(&mut context.fonts);
                let row = ((point.y - self.rect.min.y - padding).max(0) / height) as usize;
                let index = (self.first_line + row.min(rows - 1)).min(lines.len() - 1);
                let line = &lines[index];
                let text = &self.buffer.text[line.clone()];
                let x = point.x - self.rect.min.x - padding;
                let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, CURRENT_DEVICE.dpi);
                let mut previous = 0;
                self.buffer.cursor = line.start;
                for (i, c) in text.char_indices() {
                    let end = i + c.len_utf8();
                    let advance = font.plan(&text[..end], None, None).width;
                    if x < (previous + advance) / 2 { break; }
                    self.buffer.cursor = line.start + end;
                    previous = advance;
                }
                self.buffer.partial.clear();
                self.refresh(rq, &mut context.fonts);
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let (padding, height, rows, lines) = self.layout(fonts);
        let font = font_from_style(fonts, &NORMAL_STYLE, CURRENT_DEVICE.dpi);
        fb.draw_rectangle(&self.rect, WHITE);
        if self.buffer.text.is_empty() {
            let plan = font.plan("Write a note…", Some(self.rect.width() as i32 - 2 * padding), None);
            font.render(fb, BLACK, &plan, pt!(self.rect.min.x + padding,
                self.rect.min.y + padding + font.ascender()));
        }
        let cursor_line = lines.iter().rposition(|r| r.start <= self.buffer.cursor).unwrap_or(0);
        for (row, (index, range)) in lines.iter().enumerate().skip(self.first_line).take(rows).enumerate() {
            let y = self.rect.min.y + padding + row as i32 * height;
            let origin = pt!(self.rect.min.x + padding, y + font.ascender());
            let plan = font.plan(&self.buffer.text[range.clone()], None, None);
            font.render(fb, BLACK, &plan, origin);
            if index == cursor_line {
                let end = self.buffer.cursor.min(range.end);
                let x = origin.x + font.plan(&self.buffer.text[range.start..end], None, None).width;
                let thickness = scale_by_dpi(2.0, CURRENT_DEVICE.dpi) as i32;
                fb.draw_rectangle(&rect![x, y, x + thickness, y + height], BLACK);
                if !self.buffer.partial.is_empty() {
                    let plan = font.plan(&self.buffer.partial, Some((self.rect.max.x - x - padding).max(1)), None);
                    font.render(fb, BLACK, &plan, pt!(x + thickness * 2, origin.y));
                }
            }
        }
    }

    fn rect(&self) -> &Rectangle { &self.rect }
    fn rect_mut(&mut self) -> &mut Rectangle { &mut self.rect }
    fn children(&self) -> &Vec<Box<dyn View>> { &self.children }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> { &mut self.children }
    fn id(&self) -> Id { self.id }
}
