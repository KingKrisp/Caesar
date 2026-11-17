use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};
use crate::geom::{Axis, DiagDir, Dir, Rectangle, Region};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::view::Event;

macro_rules! labelled_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub fn label(self) -> &'static str { match self { $(Self::$variant => $label),+ } }
        }
    }
}

labelled_enum!(Binding {
    TapNorthWest => "Tap: top-left corner", TapNorth => "Tap: top edge",
    TapNorthEast => "Tap: top-right corner", TapWest => "Tap: left edge",
    TapCenter => "Tap: center", TapEast => "Tap: right edge",
    TapSouthWest => "Tap: bottom-left corner", TapSouth => "Tap: bottom edge",
    TapSouthEast => "Tap: bottom-right corner",
    SwipeWest => "Swipe left", SwipeEast => "Swipe right", SwipeNorth => "Swipe up", SwipeSouth => "Swipe down",
    SlantedNorthWest => "Diagonal swipe: up-left", SlantedNorthEast => "Diagonal swipe: up-right",
    SlantedSouthWest => "Diagonal swipe: down-left", SlantedSouthEast => "Diagonal swipe: down-right",
    MultiSwipeWest => "Two-finger swipe left", MultiSwipeEast => "Two-finger swipe right",
    MultiSwipeNorth => "Two-finger swipe up", MultiSwipeSouth => "Two-finger swipe down",
    HoldShort => "Hold text: short", HoldLong => "Hold text: long",
    ArrowWest => "Arrow gesture: left", ArrowEast => "Arrow gesture: right",
    ArrowNorth => "Arrow gesture: up", ArrowSouth => "Arrow gesture: down",
    CornerNorthWest => "Corner gesture: top-left", CornerNorthEast => "Corner gesture: top-right",
    CornerSouthWest => "Corner gesture: bottom-left", CornerSouthEast => "Corner gesture: bottom-right",
    MultiCornerNorthWest => "Two-finger corner: top-left", MultiCornerNorthEast => "Two-finger corner: top-right",
    MultiCornerSouthWest => "Two-finger corner: bottom-left", MultiCornerSouthEast => "Two-finger corner: bottom-right",
    MultiArrowWest => "Two-finger arrow: left", MultiArrowEast => "Two-finger arrow: right",
    MultiArrowNorth => "Two-finger arrow: up", MultiArrowSouth => "Two-finger arrow: down",
    PinchHorizontal => "Pinch horizontally", PinchVertical => "Pinch vertically", PinchDiagonal => "Pinch diagonally",
    SpreadHorizontal => "Spread horizontally", SpreadVertical => "Spread vertically", SpreadDiagonal => "Spread diagonally",
    Rotate => "Rotate gesture", Cross => "Cross gesture", Diamond => "Diamond gesture",
    Backward => "Back page button", Forward => "Forward page button",
    HoldBackward => "Hold back page button", HoldForward => "Hold forward page button",
    GlobalRefresh => "Global: two-finger SW + NE", GlobalScreenshot => "Global: two-finger NW + SE",
    GlobalLight => "Global: light button"
});

labelled_enum!(Action {
    Default => "Default", Disabled => "Disabled", PreviousPage => "Previous page", NextPage => "Next page",
    PreviousChapter => "Previous chapter", NextChapter => "Next chapter", FirstPage => "First page",
    LastPage => "Last visited page", GoToPage => "Go to page", TableOfContents => "Table of contents",
    ToggleBookmark => "Toggle bookmark", PreviousBookmark => "Previous bookmark", NextBookmark => "Next bookmark",
    PreviousAnnotation => "Previous annotation", NextAnnotation => "Next annotation", ToggleBars => "Show / hide reading controls",
    Search => "Search book", ToggleFrontlight => "Toggle frontlight", ToggleInverted => "Toggle dark mode",
    ToggleDithered => "Toggle dithering", Refresh => "Full screen refresh", Screenshot => "Take screenshot",
    Settings => "Open Settings", Notes => "Open Notes", Gallery => "Open Gallery", Back => "Back"
});

impl Binding {
    pub fn is_global(self) -> bool {
        matches!(self, Self::GlobalRefresh | Self::GlobalScreenshot | Self::GlobalLight)
    }
}

impl Action {
    pub fn is_global(self) -> bool {
        matches!(self, Self::Default | Self::Disabled | Self::ToggleFrontlight | Self::ToggleInverted |
            Self::Refresh | Self::Screenshot | Self::Settings | Self::Notes | Self::Gallery)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    #[serde(flatten)]
    pub bindings: BTreeMap<Binding, Action>,
}

impl Controls {
    pub fn action(&self, binding: Binding) -> Action {
        self.bindings.get(&binding).copied().unwrap_or(Action::Default)
    }

    pub fn set(&mut self, binding: Binding, action: Action) -> bool {
        if binding.is_global() && !action.is_global() { return false; }
        if action == Action::Default { self.bindings.remove(&binding); }
        else { self.bindings.insert(binding, action); }
        true
    }
}

pub fn reader_binding(event: &Event, rect: Rectangle, strip: f32, corner: f32) -> Option<Binding> {
    use Binding::*;
    use GestureEvent as G;
    Some(match *event {
        Event::Gesture(G::Tap(point)) if rect.includes(point) => match Region::from_point(point, rect, strip, corner) {
            Region::Corner(DiagDir::NorthWest) => TapNorthWest, Region::Corner(DiagDir::NorthEast) => TapNorthEast,
            Region::Corner(DiagDir::SouthWest) => TapSouthWest, Region::Corner(DiagDir::SouthEast) => TapSouthEast,
            Region::Strip(Dir::North) => TapNorth, Region::Strip(Dir::South) => TapSouth,
            Region::Strip(Dir::West) => TapWest, Region::Strip(Dir::East) => TapEast, Region::Center => TapCenter,
        },
        Event::Gesture(G::Swipe { dir, start, .. }) if rect.includes(start) => match dir {
            Dir::West => SwipeWest, Dir::East => SwipeEast, Dir::North => SwipeNorth, Dir::South => SwipeSouth,
        },
        Event::Gesture(G::SlantedSwipe { dir, start, .. }) if rect.includes(start) => match dir {
            DiagDir::NorthWest => SlantedNorthWest, DiagDir::NorthEast => SlantedNorthEast,
            DiagDir::SouthWest => SlantedSouthWest, DiagDir::SouthEast => SlantedSouthEast,
        },
        Event::Gesture(G::MultiSwipe { dir, .. }) => match dir {
            Dir::West => MultiSwipeWest, Dir::East => MultiSwipeEast, Dir::North => MultiSwipeNorth, Dir::South => MultiSwipeSouth,
        },
        Event::Gesture(G::HoldFingerShort(point, _)) if rect.includes(point) => HoldShort,
        Event::Gesture(G::HoldFingerLong(point, _)) if rect.includes(point) => HoldLong,
        Event::Gesture(G::Arrow { dir, .. }) => match dir {
            Dir::West => ArrowWest, Dir::East => ArrowEast, Dir::North => ArrowNorth, Dir::South => ArrowSouth,
        },
        Event::Gesture(G::MultiArrow { dir, .. }) => match dir {
            Dir::West => MultiArrowWest, Dir::East => MultiArrowEast, Dir::North => MultiArrowNorth, Dir::South => MultiArrowSouth,
        },
        Event::Gesture(G::Corner { dir, .. }) => match dir {
            DiagDir::NorthWest => CornerNorthWest, DiagDir::NorthEast => CornerNorthEast,
            DiagDir::SouthWest => CornerSouthWest, DiagDir::SouthEast => CornerSouthEast,
        },
        Event::Gesture(G::MultiCorner { dir, .. }) => match dir {
            DiagDir::NorthWest => MultiCornerNorthWest, DiagDir::NorthEast => MultiCornerNorthEast,
            DiagDir::SouthWest => MultiCornerSouthWest, DiagDir::SouthEast => MultiCornerSouthEast,
        },
        Event::Gesture(G::Pinch { axis, .. }) => match axis {
            Axis::Horizontal => PinchHorizontal, Axis::Vertical => PinchVertical, Axis::Diagonal => PinchDiagonal,
        },
        Event::Gesture(G::Spread { axis, .. }) => match axis {
            Axis::Horizontal => SpreadHorizontal, Axis::Vertical => SpreadVertical, Axis::Diagonal => SpreadDiagonal,
        },
        Event::Gesture(G::Rotate { quarter_turns, .. }) if quarter_turns != 0 => Rotate,
        Event::Gesture(G::Cross(_)) => Cross, Event::Gesture(G::Diamond(_)) => Diamond,
        Event::Gesture(G::HoldButtonShort(ButtonCode::Backward, ..)) => HoldBackward,
        Event::Gesture(G::HoldButtonShort(ButtonCode::Forward, ..)) => HoldForward,
        Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Released, .. }) => Backward,
        Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Released, .. }) => Forward,
        _ => return None,
    })
}

pub fn multitap_action(mut points: [crate::geom::Point; 2], rect: Rectangle, settings: &super::Settings) -> Option<Action> {
    let center = rect.min + pt!(rect.width() as i32 / 2, rect.height() as i32 / 2);
    if points.iter().all(|p| rect.includes(*p) && (p.x - center.x).abs() < rect.width() as i32 / 6 &&
                                               (p.y - center.y).abs() < rect.height() as i32 / 6) {
        return Some(Action::Settings);
    }
    if points[0].x > points[1].x { points.swap(0, 1); }
    let region = |point| Region::from_point(point, rect, settings.reader.strip_width, settings.reader.corner_width);
    let (binding, default) = match (region(points[0]), region(points[1])) {
        // A fixed recovery shortcut remains available even if all reader bindings are disabled.
        (Region::Corner(DiagDir::SouthWest), Region::Corner(DiagDir::NorthEast)) => (Binding::GlobalRefresh, Action::Refresh),
        (Region::Corner(DiagDir::NorthWest), Region::Corner(DiagDir::SouthEast)) => (Binding::GlobalScreenshot, Action::Screenshot),
        _ => return None,
    };
    let action = settings.controls.action(binding);
    Some(if action == Action::Default { default } else if action.is_global() { action } else { Action::Disabled })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_nine_tap_zones_and_swipe_directions() {
        let rect = rect![0, 0, 1000, 1400];
        for (point, expected) in [(pt!(5, 5), Binding::TapNorthWest), (pt!(500, 5), Binding::TapNorth),
            (pt!(995, 5), Binding::TapNorthEast), (pt!(5, 700), Binding::TapWest), (pt!(500, 700), Binding::TapCenter),
            (pt!(995, 700), Binding::TapEast), (pt!(5, 1395), Binding::TapSouthWest),
            (pt!(500, 1395), Binding::TapSouth), (pt!(995, 1395), Binding::TapSouthEast)] {
            assert_eq!(reader_binding(&Event::Gesture(GestureEvent::Tap(point)), rect, 0.6, 0.4), Some(expected));
        }
        for (dir, expected) in [(Dir::West, Binding::SwipeWest), (Dir::East, Binding::SwipeEast),
                               (Dir::North, Binding::SwipeNorth), (Dir::South, Binding::SwipeSouth)] {
            let event = Event::Gesture(GestureEvent::Swipe { dir, start: pt!(500, 700), end: pt!(100, 700) });
            assert_eq!(reader_binding(&event, rect, 0.6, 0.4), Some(expected));
        }
        assert_eq!(reader_binding(&Event::Gesture(GestureEvent::Tap(pt!(-5, 50))), rect, 0.6, 0.4), None);
    }

    #[test]
    fn overrides_round_trip_and_default_restores_legacy_behavior() {
        let mut settings = super::super::Settings::default();
        for binding in Binding::ALL {
            assert_eq!(settings.controls.action(*binding), Action::Default);
            assert!(settings.controls.set(*binding, Action::Disabled));
        }
        let text = toml::to_string(&settings).unwrap();
        let mut loaded: super::super::Settings = toml::from_str(&text).unwrap();
        assert_eq!(loaded.controls.bindings, settings.controls.bindings);
        assert!(loaded.controls.set(Binding::SwipeWest, Action::PreviousPage));
        assert_eq!(loaded.controls.action(Binding::SwipeWest), Action::PreviousPage);
        assert!(loaded.controls.set(Binding::SwipeWest, Action::Default));
        assert!(!loaded.controls.bindings.contains_key(&Binding::SwipeWest));
        assert!(!loaded.controls.set(Binding::GlobalRefresh, Action::NextPage));
    }

    #[test]
    fn global_shortcuts_can_be_disabled_but_center_recovery_cannot_be_removed() {
        let mut settings = super::super::Settings::default();
        let rect = rect![0, 0, 1000, 1400];
        let diagonal = [pt!(5, 1395), pt!(995, 5)];
        assert_eq!(multitap_action(diagonal, rect, &settings), Some(Action::Refresh));
        settings.controls.set(Binding::GlobalRefresh, Action::Disabled);
        assert_eq!(multitap_action(diagonal, rect, &settings), Some(Action::Disabled));
        settings.reader.strip_width = 0.9; settings.reader.corner_width = 0.9;
        for binding in Binding::ALL { settings.controls.set(*binding, Action::Disabled); }
        assert_eq!(multitap_action([pt!(450, 680), pt!(550, 720)], rect, &settings), Some(Action::Settings));
    }
}
