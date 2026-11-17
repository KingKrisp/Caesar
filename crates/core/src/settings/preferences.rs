use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use anyhow::{bail, Error};
use super::{Settings, RotationLock};
use super::controls::{Action, Binding};
use crate::metadata::TextAlign;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Preference {
    Frontlight, Brightness, Warmth, Inverted, Rotation, SleepCover, Sleep, PowerOff, AutoShare,
    TimeFormat, DateFormat, Keyboard, FontFamily, FontSize, Margin, LineHeight, Alignment,
    IgnoreCss, Continuous, Refresh, RefreshInverted, Thumbnails, FirstColumn, SecondColumn,
    StartupImport, UsbImport, AddressBar, NavigationBar, StripWidth, CornerWidth,
    Bind(Binding), ResetBindings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value { Bool(bool), Number(f32), Text(String), Action(Action) }

impl Preference {
    pub fn label(self) -> &'static str {
        use Preference::*;
        match self {
            Frontlight => "Frontlight", Brightness => "Brightness", Warmth => "Warmth", Inverted => "Dark mode",
            Rotation => "Screen orientation", SleepCover => "Sleep when cover closes", Sleep => "Sleep after inactivity",
            PowerOff => "Power off while asleep", AutoShare => "Connect USB automatically",
            TimeFormat => "Clock format", DateFormat => "Date format", Keyboard => "Keyboard layout",
            FontFamily => "Reading font", FontSize => "Font size", Margin => "Side margins",
            LineHeight => "Line spacing", Alignment => "Text alignment", IgnoreCss => "Override book styles",
            Continuous => "Continuous PDF paging", Refresh => "Screen refresh", RefreshInverted => "Dark mode screen refresh",
            Thumbnails => "Book thumbnails", FirstColumn => "Book labels", SecondColumn => "Book details",
            StartupImport => "Scan for books at startup", UsbImport => "Scan after USB disconnects", AddressBar => "Folder path bar",
            NavigationBar => "Folder navigation", StripWidth => "Edge touch area",
            CornerWidth => "Corner touch area", Bind(binding) => binding.label(), ResetBindings => "Reset gestures and shortcuts",
        }
    }

    pub fn help(self) -> &'static [&'static str] {
        use Preference::*;
        match self {
            Frontlight => &["Turn the screen's built-in light on or off."],
            Brightness => &["Adjust how brightly the screen is lit."],
            Warmth => &["Higher values give the light a warmer, amber tone."],
            Inverted => &["Use light text on a dark background."],
            Rotation => &["Choose portrait or landscape screen orientation."],
            SleepCover => &["Closing a compatible magnetic cover puts the reader to sleep."],
            Sleep => &["Put the reader to sleep after this long without input."],
            PowerOff => &["Turn off after sleeping for this long.", "Turning it back on requires a full startup."],
            AutoShare => &["Share storage with a connected computer without a prompt.", "The reading app must be running to detect USB."],
            TimeFormat => &["Choose how the time appears in the top bar."],
            DateFormat => &["Choose how dates appear on the reader."],
            Keyboard => &["Choose the on-screen keyboard layout."],
            FontFamily => &["Font used for books without saved reading settings."],
            FontSize => &["Text size in points for books without saved settings."],
            Margin => &["Space at each side of the page, in millimetres."],
            LineHeight => &["Space between lines, relative to the text size."],
            Alignment => &["Justified text lines up with both page edges."],
            IgnoreCss => &["Use the reader's formatting instead of the book's styles."],
            Continuous => &["For PDFs and other fixed-layout documents fitted to width.", "Continue down the page before moving to the next page."],
            Refresh | RefreshInverted => &["A full refresh briefly flashes to clear faded text.", "More often reduces ghosting; less often flashes less."],
            Thumbnails => &["Show cover previews alongside books in the library."],
            FirstColumn => &["Choose the main label shown for each book."],
            SecondColumn => &["Show reading progress or publication year beside each book."],
            StartupImport => &["Look for added or removed books when the app starts."],
            UsbImport => &["Update the library after you finish transferring files."],
            AddressBar => &["Show the current folder path in the library."],
            NavigationBar => &["Show folders for browsing your library."],
            StripWidth => &["Width of the touch areas along the screen edges.", "Measured as a share of the screen's shorter side."],
            CornerWidth => &["Size of the triangular touch areas in the corners.", "Measured as a share of the screen's shorter side."],
            Bind(binding) if binding.is_global() => &["Choose the shortcut's action throughout the app."],
            Bind(_) => &["Choose what this gesture or button does while reading."],
            ResetBindings => &["Restore the original reading and global shortcut actions."],
        }
    }

    pub fn value_label(self, value: &Value) -> String {
        use Preference::*;
        let number = |v: f32| { let s = format!("{:.1}", v); s.strip_suffix(".0").unwrap_or(&s).to_string() };
        if let Value::Number(v) = value {
            let n = number(*v);
            return match self {
                Brightness | Warmth => format!("{}%", n),
                Sleep if *v == 0. => "Never".into(),
                Sleep => format!("{} {}", n, if *v == 1. { "minute" } else { "minutes" }),
                PowerOff if *v == 0. => "Never".into(),
                PowerOff => format!("{} {}", n, if *v == 1. { "day" } else { "days" }),
                Refresh | RefreshInverted if *v == 0. => "No periodic refresh".into(),
                Refresh | RefreshInverted if *v == 1. => "Every page turn".into(),
                Refresh | RefreshInverted => format!("Every {} page turns", n),
                FontSize => format!("{} pt", n), Margin => format!("{} mm", n),
                LineHeight => format!("{}×", n),
                StripWidth | CornerWidth => format!("{}%", number(*v * 50.)),
                _ => n,
            };
        }
        value.label()
    }

    pub fn current(self, s: &Settings) -> Value {
        use Preference::*;
        match self {
            Frontlight => Value::Bool(s.frontlight), Brightness => Value::Number(s.frontlight_levels.intensity),
            Warmth => Value::Number(s.frontlight_levels.warmth), Inverted => Value::Bool(s.inverted),
            Rotation => Value::Text(match s.rotation_lock { Some(RotationLock::Portrait) => "Portrait",
                Some(RotationLock::Landscape) => "Landscape",
                _ if crate::device::CURRENT_DEVICE.has_gyroscope() => "Automatic", _ => "Portrait" }.to_string()),
            SleepCover => Value::Bool(s.sleep_cover), Sleep => Value::Number(s.auto_suspend), PowerOff => Value::Number(s.auto_power_off),
            AutoShare => Value::Bool(s.auto_share), TimeFormat => Value::Text(s.time_format.clone()), DateFormat => Value::Text(s.date_format.clone()),
            Keyboard => Value::Text(s.keyboard_layout.clone()), FontFamily => Value::Text(s.reader.font_family.clone()),
            FontSize => Value::Number(s.reader.font_size), Margin => Value::Number(s.reader.margin_width as f32),
            LineHeight => Value::Number(s.reader.line_height), Alignment => Value::Text(s.reader.text_align.to_string()),
            IgnoreCss => Value::Bool(s.reader.ignore_document_css), Continuous => Value::Bool(s.reader.continuous_fit_to_width),
            Refresh => Value::Number(s.reader.refresh_rate.global.regular as f32),
            RefreshInverted => Value::Number(s.reader.refresh_rate.global.inverted as f32),
            Thumbnails => Value::Bool(s.libraries[s.selected_library].thumbnail_previews),
            FirstColumn => Value::Text(match s.libraries[s.selected_library].first_column {
                super::FirstColumn::TitleAndAuthor => "Title and author", super::FirstColumn::FileName => "File name" }.to_string()),
            SecondColumn => Value::Text(format!("{:?}", s.libraries[s.selected_library].second_column)),
            StartupImport => Value::Bool(s.import.startup_trigger), UsbImport => Value::Bool(s.import.unshare_trigger),
            AddressBar => Value::Bool(s.home.address_bar), NavigationBar => Value::Bool(s.home.navigation_bar),
            StripWidth => Value::Number(s.reader.strip_width), CornerWidth => Value::Number(s.reader.corner_width),
            Bind(binding) => Value::Action(s.controls.action(binding)), ResetBindings => Value::Text("Restore".to_string()),
        }
    }
}

impl Value {
    pub fn label(&self) -> String {
        match self { Self::Bool(v) => if *v { "On" } else { "Off" }.to_string(),
            Self::Number(v) => format!("{}", v),
            Self::Text(v) => v.clone(), Self::Action(v) => v.label().to_string() }
    }
}

pub fn apply(settings: &mut Settings, preference: Preference, value: &Value) -> Result<(), Error> {
    use Preference::*;
    use Value::*;
    match (preference, value) {
        (Frontlight, Bool(v)) => settings.frontlight = *v,
        (Inverted, Bool(v)) => settings.inverted = *v,
        (SleepCover, Bool(v)) => settings.sleep_cover = *v,
        (AutoShare, Bool(v)) => settings.auto_share = *v,
        (IgnoreCss, Bool(v)) => settings.reader.ignore_document_css = *v,
        (Continuous, Bool(v)) => settings.reader.continuous_fit_to_width = *v,
        (StartupImport, Bool(v)) => settings.import.startup_trigger = *v,
        (UsbImport, Bool(v)) => settings.import.unshare_trigger = *v,
        (AddressBar, Bool(v)) => settings.home.address_bar = *v,
        (NavigationBar, Bool(v)) => settings.home.navigation_bar = *v,
        (Thumbnails, Bool(v)) => settings.libraries[settings.selected_library].thumbnail_previews = *v,
        (Brightness, Number(v)) if v.is_finite() && (0.0..=100.0).contains(v) => settings.frontlight_levels.intensity = *v,
        (Warmth, Number(v)) if v.is_finite() && (0.0..=100.0).contains(v) => settings.frontlight_levels.warmth = *v,
        (Sleep, Number(v)) if v.is_finite() && (0.0..=1440.0).contains(v) => settings.auto_suspend = *v,
        (PowerOff, Number(v)) if v.is_finite() && (0.0..=365.0).contains(v) => settings.auto_power_off = *v,
        (FontSize, Number(v)) if v.is_finite() && (5.0..=40.0).contains(v) => settings.reader.font_size = *v,
        (Margin, Number(v)) if v.is_finite() && (0.0..=30.0).contains(v) => settings.reader.margin_width = *v as i32,
        (LineHeight, Number(v)) if v.is_finite() && (0.8..=3.0).contains(v) => settings.reader.line_height = *v,
        (Refresh, Number(v)) if v.is_finite() && (0.0..=32.0).contains(v) => settings.reader.refresh_rate.global.regular = *v as u8,
        (RefreshInverted, Number(v)) if v.is_finite() && (0.0..=32.0).contains(v) => settings.reader.refresh_rate.global.inverted = *v as u8,
        (StripWidth, Number(v)) if v.is_finite() && (0.1..=0.9).contains(v) => settings.reader.strip_width = *v,
        (CornerWidth, Number(v)) if v.is_finite() && (0.1..=0.9).contains(v) => settings.reader.corner_width = *v,
        (Keyboard, Text(v)) if !v.is_empty() => settings.keyboard_layout = v.clone(),
        (FontFamily, Text(v)) if !v.is_empty() => settings.reader.font_family = v.clone(),
        (TimeFormat, Text(v)) if ["%H:%M", "%I:%M %p"].contains(&v.as_str()) => settings.time_format = v.clone(),
        (DateFormat, Text(v)) if ["%A, %B %-d, %Y", "%Y-%m-%d", "%d/%m/%Y"].contains(&v.as_str()) => settings.date_format = v.clone(),
        (Rotation, Text(v)) => settings.rotation_lock = match v.as_str() {
            "Portrait" => Some(RotationLock::Portrait), "Landscape" => Some(RotationLock::Landscape),
            "Automatic" => None, _ => bail!("Invalid orientation"),
        },
        (Alignment, Text(v)) => settings.reader.text_align = match v.as_str() {
            "Left" => TextAlign::Left, "Right" => TextAlign::Right, "Center" => TextAlign::Center,
            "Justify" => TextAlign::Justify, _ => bail!("Invalid alignment"),
        },
        (FirstColumn, Text(v)) => settings.libraries[settings.selected_library].first_column = match v.as_str() {
            "Title and author" => super::FirstColumn::TitleAndAuthor, "File name" => super::FirstColumn::FileName,
            _ => bail!("Invalid column"),
        },
        (SecondColumn, Text(v)) => settings.libraries[settings.selected_library].second_column = match v.as_str() {
            "Progress" => super::SecondColumn::Progress, "Year" => super::SecondColumn::Year, _ => bail!("Invalid column"),
        },
        (Bind(binding), Action(action)) if settings.controls.set(binding, *action) => (),
        (ResetBindings, Text(v)) if v == "Restore" => {
            settings.controls.bindings.clear();
            // The physical inversion stays separate from logical reader bindings.
        },
        _ => bail!("Invalid setting value"),
    }
    Ok(())
}

pub fn save(settings: &Settings, path: &Path) -> Result<(), Error> {
    let text = toml::to_string(settings)?;
    let temp = path.with_extension(format!("{}.tmp", chrono::Local::now().timestamp_nanos_opt().unwrap_or_default()));
    let mut file = OpenOptions::new().create_new(true).write(true).open(&temp)?;
    let result = (|| -> Result<(), Error> {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() { fs::remove_file(temp).ok(); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_values_show_units_and_refresh_meaning() {
        assert_eq!(Preference::Refresh.value_label(&Value::Number(10.)), "Every 10 page turns");
        assert_eq!(Preference::Refresh.value_label(&Value::Number(0.)), "No periodic refresh");
        assert_eq!(Preference::Sleep.value_label(&Value::Number(0.)), "Never");
        assert_eq!(Preference::PowerOff.value_label(&Value::Number(1.5)), "1.5 days");
        assert_eq!(Preference::Brightness.value_label(&Value::Number(10.)), "10%");
        assert_eq!(Preference::StripWidth.value_label(&Value::Number(0.6)), "30%");
        assert_eq!(Preference::Margin.value_label(&Value::Number(8.)), "8 mm");
    }

    #[test]
    fn legacy_network_preferences_and_fetcher_hooks_cannot_enable_online_features() {
        let settings: Settings = toml::from_str("wifi = true\nexternal-urls-queue = 'urls.txt'\n[[libraries]]\nname = 'Books'\n[[libraries.hooks]]\npath = 'Articles'\nprogram = 'network-fetcher'\n").unwrap();
        assert!(crate::settings::OFFLINE);
        assert!(settings.libraries[0].hooks.is_empty());
        let saved = toml::to_string(&settings).unwrap();
        assert!(!saved.contains("wifi ="));
        assert!(!saved.contains("external-urls-queue"));
        assert!(!saved.contains("network-fetcher"));
    }

    #[test]
    fn rejects_invalid_values_without_changing_preferences() {
        let mut settings = Settings::default();
        let original = toml::to_string(&settings).unwrap();
        for (preference, value) in [(Preference::Brightness, Value::Number(101.)),
            (Preference::Brightness, Value::Number(f32::NAN)), (Preference::Sleep, Value::Number(-1.)),
            (Preference::CornerWidth, Value::Number(1.)), (Preference::FontSize, Value::Number(0.)),
            (Preference::Rotation, Value::Text("invalid".into())),
            (Preference::Bind(Binding::GlobalLight), Value::Action(Action::NextPage))] {
            assert!(apply(&mut settings, preference, &value).is_err());
            assert_eq!(toml::to_string(&settings).unwrap(), original);
        }
    }

    #[test]
    fn settings_and_bindings_survive_immediate_save_and_reload() {
        let mut settings = Settings::default();
        apply(&mut settings, Preference::Brightness, &Value::Number(70.)).unwrap();
        apply(&mut settings, Preference::Sleep, &Value::Number(5.)).unwrap();
        apply(&mut settings, Preference::Bind(Binding::TapEast), &Value::Action(Action::Disabled)).unwrap();
        let directory = std::env::temp_dir().join(format!("caesar-settings-{}-{}", std::process::id(), chrono::Local::now().timestamp_nanos_opt().unwrap()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("Settings.toml");
        fs::write(&path, "old settings").unwrap();
        save(&settings, &path).unwrap();
        let loaded: Settings = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.frontlight_levels.intensity, 70.);
        assert_eq!(loaded.auto_suspend, 5.);
        assert_eq!(loaded.controls.action(Binding::TapEast), Action::Disabled);
        let saved = fs::read_to_string(&path).unwrap();
        assert!(save(&settings, &directory.join("missing/Settings.toml")).is_err());
        assert_eq!(saved, fs::read_to_string(path).unwrap());
        fs::remove_dir_all(directory).unwrap();
    }
}
