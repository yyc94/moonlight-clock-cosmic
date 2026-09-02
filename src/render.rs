use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::font::{Family, Style as FontStyle, Weight};
use cosmic::iced::mouse;
use cosmic::iced::widget::canvas::path::Arc;
use cosmic::iced::widget::canvas::{self, Frame, Geometry, LineCap, Path, Stroke, Text};
use cosmic::iced::{Color, Font, Pixels, Point, Radians, Rectangle};
use cosmic::{Renderer, Theme};

use crate::config::{AppConfig, FontConfig, Palette};
use crate::content::{ClockContent, Icon};
use crate::weather::WeatherIcon;

#[derive(Clone)]
pub struct ClockCanvas {
    pub config: AppConfig,
    pub content: ClockContent,
}

impl<Message> canvas::Program<Message, Theme, Renderer> for ClockCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.scale(self.config.window.scale);
        draw_clock(&mut frame, &self.config, &self.content);
        vec![frame.into_geometry()]
    }
}

fn draw_clock(frame: &mut Frame<Renderer>, config: &AppConfig, content: &ClockContent) {
    let palette = config.palette();
    draw_background(frame, &palette);

    let date_size = fit_size(&content.date_text, config.appearance.date_font.size, 330.0);
    let time_size = fit_size(&content.time_text, config.appearance.time_font.size, 440.0);
    let caption_size = fit_multiline(
        &content.caption,
        config.appearance.caption_font.size * 0.7,
        270.0,
    );
    let heading_size = fit_size(
        &content.heading,
        config.appearance.caption_font.size * 0.8,
        260.0,
    );

    if config.appearance.time_shadow {
        let offset = config.appearance.time_shadow_offset;
        draw_text(
            frame,
            Point::new(590.0 - offset, 61.0 + offset),
            &content.time_text,
            &config.appearance.time_font,
            time_size,
            color(&palette.shadow),
            Horizontal::Right,
        );
    }
    draw_text(
        frame,
        Point::new(590.0, 61.0),
        &content.time_text,
        &config.appearance.time_font,
        time_size,
        color(&palette.time),
        Horizontal::Right,
    );
    draw_text(
        frame,
        Point::new(468.0, 4.0),
        &content.date_text,
        &config.appearance.date_font,
        date_size,
        color(&palette.date),
        Horizontal::Right,
    );
    if !content.weekday.is_empty() {
        draw_text(
            frame,
            Point::new(494.0, 12.0),
            "•",
            &FontConfig {
                family: "serif".into(),
                size: 38.0,
                weight: 700,
                style: "normal".into(),
            },
            38.0,
            color(&palette.date),
            Horizontal::Center,
        );
        draw_text(
            frame,
            Point::new(590.0, 22.0),
            &content.weekday,
            &FontConfig {
                family: config.appearance.date_font.family.clone(),
                size: 31.0,
                weight: 400,
                style: "normal".into(),
            },
            31.0,
            color(&palette.date),
            Horizontal::Right,
        );
    }

    let shadow = config.appearance.caption_shadow;
    let offset = config.appearance.caption_shadow_offset;
    if !content.caption.is_empty() {
        if shadow {
            draw_text(
                frame,
                Point::new(507.0 - offset, 181.0 + offset),
                &content.caption,
                &config.appearance.caption_font,
                caption_size,
                color(&palette.shadow),
                Horizontal::Right,
            );
        }
        draw_text(
            frame,
            Point::new(507.0, 181.0),
            &content.caption,
            &config.appearance.caption_font,
            caption_size,
            color(&palette.bottom),
            Horizontal::Right,
        );
    }
    if !content.heading.is_empty() {
        if shadow {
            draw_text(
                frame,
                Point::new(505.0 - offset, 178.0 + offset),
                &content.heading,
                &config.appearance.caption_font,
                heading_size,
                color(&palette.shadow),
                Horizontal::Right,
            );
        }
        draw_text(
            frame,
            Point::new(505.0, 178.0),
            &content.heading,
            &config.appearance.caption_font,
            heading_size,
            color(&palette.bottom),
            Horizontal::Right,
        );
    }

    let (number_size, number_x, unit_size, unit_x) = number_layout(config, content);
    let (number_top, icon_y) = bottom_positions(config);
    if !content.number.is_empty() {
        let number_color = if config.appearance.highlight_numbers {
            color(&palette.highlight)
        } else {
            color(&palette.bottom)
        };
        if shadow {
            draw_text(
                frame,
                Point::new(number_x - offset, number_top + offset),
                &content.number,
                &config.appearance.caption_font,
                number_size,
                color(&palette.shadow),
                Horizontal::Right,
            );
        }
        draw_text(
            frame,
            Point::new(number_x, number_top),
            &content.number,
            &config.appearance.caption_font,
            number_size,
            number_color,
            Horizontal::Right,
        );
    }
    if !content.unit.is_empty() {
        draw_text(
            frame,
            Point::new(unit_x, number_top + number_size - unit_size),
            &content.unit,
            &config.appearance.caption_font,
            unit_size,
            color(&palette.bottom),
            Horizontal::Right,
        );
    }
    if let Some(icon) = &content.icon {
        draw_icon(
            frame,
            bottom_icon_x(config),
            icon_y,
            icon,
            config.appearance.emoji_size,
        );
    }
    if !content.secondary.is_empty() {
        let secondary_size = fit_multiline(
            &content.secondary,
            config.appearance.caption_font.size * 0.34,
            310.0,
        );
        draw_text(
            frame,
            Point::new(584.0, 309.0),
            &content.secondary,
            &config.appearance.caption_font,
            secondary_size,
            color(&palette.bottom),
            Horizontal::Right,
        );
    }
    if content.warning {
        draw_text(
            frame,
            Point::new(607.0, 180.0),
            "⚠",
            &FontConfig::default(),
            23.0,
            Color::from_rgb8(255, 209, 102),
            Horizontal::Right,
        );
    }
}

fn draw_background(frame: &mut Frame<Renderer>, palette: &Palette) {
    let top = Path::new(|path| {
        path.move_to(Point::new(614.0, 189.0));
        path.line_to(Point::new(614.0, 0.0));
        path.line_to(Point::new(0.0, 0.0));
        path.bezier_curve_to(
            Point::new(66.0, 7.0),
            Point::new(170.0, 37.0),
            Point::new(274.0, 70.0),
        );
        path.bezier_curve_to(
            Point::new(429.0, 120.0),
            Point::new(584.0, 180.0),
            Point::new(614.0, 189.0),
        );
        path.close();
    });
    let mut top_start = color(&palette.corner1);
    top_start.a = 0.65;
    frame.fill(
        &top,
        canvas::gradient::Linear::new(Point::ORIGIN, Point::new(614.0, 189.0))
            .add_stop(0.0, top_start)
            .add_stop(1.0, color(&palette.corner1)),
    );

    let lower = Path::new(|path| {
        path.move_to(Point::new(614.0, 189.0));
        path.line_to(Point::new(614.0, 387.0));
        path.bezier_curve_to(
            Point::new(558.0, 312.0),
            Point::new(452.0, 181.0),
            Point::new(274.0, 70.0),
        );
        path.bezier_curve_to(
            Point::new(377.0, 101.0),
            Point::new(584.0, 180.0),
            Point::new(614.0, 189.0),
        );
        path.close();
    });
    let mut lower_end = color(&palette.corner2);
    lower_end.a = 0.4;
    frame.fill(
        &lower,
        canvas::gradient::Linear::new(Point::new(274.0, 70.0), Point::new(614.0, 355.0))
            .add_stop(0.0, color(&palette.corner2))
            .add_stop(1.0, lower_end),
    );
}

fn draw_text(
    frame: &mut Frame<Renderer>,
    position: Point,
    value: &str,
    config: &FontConfig,
    size: f32,
    color: Color,
    align_x: Horizontal,
) {
    if value.is_empty() {
        return;
    }
    frame.fill_text(Text {
        content: value.into(),
        position,
        color,
        size: Pixels(size),
        font: font(config),
        align_x: align_x.into(),
        align_y: Vertical::Top,
        ..Text::default()
    });
}

fn font(config: &FontConfig) -> Font {
    let family = match config.family.to_lowercase().as_str() {
        "sans" | "sans-serif" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" | "mono" => Family::Monospace,
        "cursive" => Family::Cursive,
        "fantasy" => Family::Fantasy,
        _ => Family::Name(intern_font_name(&config.family)),
    };
    let weight = match config.weight {
        0..=199 => Weight::Thin,
        200..=299 => Weight::ExtraLight,
        300..=399 => Weight::Light,
        400..=499 => Weight::Normal,
        500..=599 => Weight::Medium,
        600..=699 => Weight::Semibold,
        700..=799 => Weight::Bold,
        800..=899 => Weight::ExtraBold,
        _ => Weight::Black,
    };
    let style = match config.style.as_str() {
        "italic" => FontStyle::Italic,
        "oblique" => FontStyle::Oblique,
        _ => FontStyle::Normal,
    };
    Font {
        family,
        weight,
        style,
        ..Font::DEFAULT
    }
}

fn intern_font_name(name: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let names = NAMES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut names = names.lock().unwrap();
    if let Some(value) = names.get(name) {
        return value;
    }
    let value = Box::leak(name.to_owned().into_boxed_str());
    names.insert(name.into(), value);
    value
}

fn number_layout(config: &AppConfig, content: &ClockContent) -> (f32, f32, f32, f32) {
    let requested = config.appearance.caption_font.size;
    if matches!(
        config.bottom.mode.as_str(),
        "weather-temp-c" | "weather-temp-f"
    ) && content.icon.is_some()
        && !content.number.is_empty()
    {
        let unit_size = requested * 0.68;
        let row_right = bottom_icon_x(config) - 60.0;
        let unit_width = text_units(&content.unit) * unit_size * 0.8;
        return (
            requested,
            row_right - unit_width - 2.0,
            unit_size,
            row_right,
        );
    }
    if content.icon.is_none() || content.number.is_empty() {
        let size = fit_size(&(content.number.clone() + &content.unit), requested, 230.0);
        return (size, 505.0, size, 535.0);
    }
    let icon_left = bottom_icon_x(config) - config.appearance.emoji_size * 0.55;
    let row_right = 535.0_f32.min(icon_left - 10.0);
    let size = fit_size(
        &(content.number.clone() + &content.unit),
        requested,
        (row_right - 310.0).max(70.0),
    );
    let unit_width = text_units(&content.unit) * size * 0.8;
    (size, row_right - unit_width - 4.0, size, row_right)
}

fn bottom_positions(config: &AppConfig) -> (f32, f32) {
    if matches!(
        config.bottom.mode.as_str(),
        "weather-temp-c" | "weather-temp-f"
    ) {
        (180.0, 210.0)
    } else {
        (237.0, 254.0)
    }
}

fn bottom_icon_x(config: &AppConfig) -> f32 {
    if config.bottom.mode.starts_with("weather") {
        568.0
    } else {
        580.0
    }
}

fn draw_icon(frame: &mut Frame<Renderer>, x: f32, y: f32, icon: &Icon, size: f32) {
    let scale = size / 70.0;
    match icon {
        Icon::Moon(phase) => draw_moon(frame, x, y, phase, scale),
        Icon::Weather(icon) => draw_weather(frame, x, y, *icon, scale),
        Icon::Warning => draw_warning(frame, x, y, scale),
    }
}

fn draw_moon(frame: &mut Frame<Renderer>, x: f32, y: f32, phase: &str, scale: f32) {
    let dark = Color::from_rgb8(52, 56, 63);
    let light = Color::from_rgb8(244, 220, 98);
    let rim = Color::from_rgb8(115, 121, 130);
    let radius = 29.0 * scale;
    let base = Path::circle(Point::new(x, y), radius);
    let base_color = if matches!(phase, "full" | "first-full" | "full-last") {
        light
    } else {
        dark
    };
    frame.fill(&base, base_color);

    match phase {
        "new-first" => frame.fill(&moon_lobe(x, y, radius, 1.0, 1.0), light),
        "first" => frame.fill(&moon_lobe(x, y, radius, 1.0, 0.0), light),
        "first-full" => frame.fill(&moon_lobe(x, y, radius, -1.0, 1.0), dark),
        "full-last" => frame.fill(&moon_lobe(x, y, radius, 1.0, 1.0), dark),
        "last" => frame.fill(&moon_lobe(x, y, radius, -1.0, 0.0), light),
        "last-new" => frame.fill(&moon_lobe(x, y, radius, -1.0, 1.0), light),
        _ => {}
    }
    frame.stroke(
        &base,
        Stroke::default().with_color(rim).with_width(2.0 * scale),
    );
}

fn moon_lobe(x: f32, y: f32, radius: f32, side: f32, inset: f32) -> Path {
    let center = Point::new(x, y);
    let top = Point::new(x, y - radius);
    let bottom = Point::new(x, y + radius);
    Path::new(|path| {
        if side > 0.0 {
            path.move_to(top);
            path.arc(Arc {
                center,
                radius,
                start_angle: Radians(-std::f32::consts::FRAC_PI_2),
                end_angle: Radians(std::f32::consts::FRAC_PI_2),
            });
            path.bezier_curve_to(
                Point::new(x + inset * radius, y + radius),
                Point::new(x + inset * radius, y - radius),
                top,
            );
        } else {
            path.move_to(bottom);
            path.arc(Arc {
                center,
                radius,
                start_angle: Radians(std::f32::consts::FRAC_PI_2),
                end_angle: Radians(3.0 * std::f32::consts::FRAC_PI_2),
            });
            path.bezier_curve_to(
                Point::new(x - inset * radius, y - radius),
                Point::new(x - inset * radius, y + radius),
                bottom,
            );
        }
        path.close();
    })
}

fn draw_weather(frame: &mut Frame<Renderer>, x: f32, y: f32, icon: WeatherIcon, scale: f32) {
    match icon {
        WeatherIcon::Sun => draw_sun(frame, x, y, scale),
        WeatherIcon::Thermometer => draw_thermometer(frame, x, y, scale),
        _ => draw_cloud_weather(frame, x, y, icon, scale),
    }
}

fn draw_sun(frame: &mut Frame<Renderer>, x: f32, y: f32, scale: f32) {
    let yellow = Color::from_rgb8(255, 212, 90);
    frame.fill(&Path::circle(Point::new(x, y), 19.0 * scale), yellow);
    let stroke = Stroke::default()
        .with_color(yellow)
        .with_width(4.0 * scale)
        .with_line_cap(LineCap::Round);
    for angle in (0..360).step_by(45) {
        let radians = (angle as f32).to_radians();
        let from = Point::new(
            x + radians.sin() * 25.0 * scale,
            y - radians.cos() * 25.0 * scale,
        );
        let to = Point::new(
            x + radians.sin() * 31.0 * scale,
            y - radians.cos() * 31.0 * scale,
        );
        frame.stroke(&Path::line(from, to), stroke);
    }
}

fn draw_cloud_weather(frame: &mut Frame<Renderer>, x: f32, y: f32, icon: WeatherIcon, scale: f32) {
    if icon == WeatherIcon::PartlyCloudy {
        frame.fill(
            &Path::circle(Point::new(x - 15.0 * scale, y - 13.0 * scale), 15.0 * scale),
            Color::from_rgb8(255, 212, 90),
        );
    }
    let cloud = Path::new(|path| {
        path.move_to(Point::new(x - 25.0 * scale, y + 24.0 * scale));
        path.bezier_curve_to(
            Point::new(x - 36.0 * scale, y + 23.0 * scale),
            Point::new(x - 37.0 * scale, y + 7.0 * scale),
            Point::new(x - 27.0 * scale, y + 4.0 * scale),
        );
        path.bezier_curve_to(
            Point::new(x - 26.0 * scale, y - 8.0 * scale),
            Point::new(x - 15.0 * scale, y - 14.0 * scale),
            Point::new(x - 5.0 * scale, y - 10.0 * scale),
        );
        path.bezier_curve_to(
            Point::new(x + 5.0 * scale, y - 27.0 * scale),
            Point::new(x + 30.0 * scale, y - 18.0 * scale),
            Point::new(x + 31.0 * scale, y + 2.0 * scale),
        );
        path.bezier_curve_to(
            Point::new(x + 43.0 * scale, y + 7.0 * scale),
            Point::new(x + 38.0 * scale, y + 24.0 * scale),
            Point::new(x + 24.0 * scale, y + 24.0 * scale),
        );
        path.close();
    });
    frame.fill(&cloud, Color::from_rgb8(232, 241, 247));
    frame.stroke(
        &cloud,
        Stroke::default()
            .with_color(Color::from_rgb8(170, 185, 196))
            .with_width(2.0 * scale),
    );

    let stroke = Stroke::default()
        .with_width(4.0 * scale)
        .with_line_cap(LineCap::Round);
    match icon {
        WeatherIcon::Fog => {
            let fog = stroke.with_color(Color::from_rgb8(185, 203, 213));
            for (x1, y1, x2) in [(-25.0, 32.0, 11.0), (-11.0, 40.0, 23.0)] {
                frame.stroke(
                    &Path::line(
                        Point::new(x + x1 * scale, y + y1 * scale),
                        Point::new(x + x2 * scale, y + y1 * scale),
                    ),
                    fog,
                );
            }
        }
        WeatherIcon::Shower | WeatherIcon::Rain => {
            let rain = stroke.with_color(Color::from_rgb8(97, 184, 239));
            for x1 in [-15.0, 1.0, 17.0] {
                frame.stroke(
                    &Path::line(
                        Point::new(x + x1 * scale, y + 30.0 * scale),
                        Point::new(x + (x1 - 4.0) * scale, y + 38.0 * scale),
                    ),
                    rain,
                );
            }
        }
        WeatherIcon::Snow => {
            for (x1, y1) in [(-15.0, 34.0), (0.0, 39.0), (16.0, 34.0)] {
                frame.fill(
                    &Path::circle(Point::new(x + x1 * scale, y + y1 * scale), 3.0 * scale),
                    Color::from_rgb8(189, 231, 255),
                );
            }
        }
        WeatherIcon::Storm => {
            let bolt = Path::new(|path| {
                for (index, (px, py)) in [
                    (3.0, 25.0),
                    (16.0, 25.0),
                    (7.0, 38.0),
                    (16.0, 38.0),
                    (-2.0, 52.0),
                    (2.0, 35.0),
                    (-7.0, 35.0),
                ]
                .into_iter()
                .enumerate()
                {
                    let point = Point::new(x + px * scale, y + py * scale);
                    if index == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                path.close();
            });
            frame.fill(&bolt, Color::from_rgb8(255, 212, 90));
        }
        _ => {}
    }
}

fn draw_thermometer(frame: &mut Frame<Renderer>, x: f32, y: f32, scale: f32) {
    let blue = Color::from_rgb8(97, 184, 239);
    frame.stroke(
        &Path::line(
            Point::new(x, y - 23.0 * scale),
            Point::new(x, y + 15.0 * scale),
        ),
        Stroke::default()
            .with_color(blue)
            .with_width(10.0 * scale)
            .with_line_cap(LineCap::Round),
    );
    frame.fill(
        &Path::circle(Point::new(x, y + 22.0 * scale), 13.0 * scale),
        blue,
    );
}

fn draw_warning(frame: &mut Frame<Renderer>, x: f32, y: f32, scale: f32) {
    let triangle = Path::new(|path| {
        path.move_to(Point::new(x, y - 31.0 * scale));
        path.line_to(Point::new(x + 34.0 * scale, y + 27.0 * scale));
        path.line_to(Point::new(x - 34.0 * scale, y + 27.0 * scale));
        path.close();
    });
    frame.fill(&triangle, Color::from_rgb8(255, 209, 102));
    frame.stroke(
        &Path::line(
            Point::new(x, y - 14.0 * scale),
            Point::new(x, y + 9.0 * scale),
        ),
        Stroke::default()
            .with_color(Color::from_rgb8(59, 51, 32))
            .with_width(6.0 * scale)
            .with_line_cap(LineCap::Round),
    );
}

fn fit_size(text: &str, requested: f32, max_width: f32) -> f32 {
    if text.is_empty() {
        return requested;
    }
    let estimate = (text_units(text) * requested).max(1.0);
    (requested * (max_width / estimate).min(1.0)).max(10.0)
}

fn fit_multiline(text: &str, requested: f32, max_width: f32) -> f32 {
    text.lines()
        .map(|line| fit_size(line, requested, max_width))
        .reduce(f32::min)
        .unwrap_or(requested)
}

fn text_units(text: &str) -> f32 {
    text.chars()
        .map(|value| if value as u32 > 0x2ff { 1.0 } else { 0.58 })
        .sum()
}

fn color(value: &str) -> Color {
    let value = value.trim().trim_start_matches('#');
    if value.len() == 6
        && let Ok(raw) = u32::from_str_radix(value, 16)
    {
        return Color::from_rgb8((raw >> 16) as u8, (raw >> 8) as u8, raw as u8);
    }
    Color::WHITE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_layout_preserves_right_edge_inset() {
        let mut config = AppConfig::default();
        config.bottom.mode = "weather-temp-c".into();
        let content = ClockContent {
            number: "26".into(),
            unit: "°C".into(),
            icon: Some(Icon::Weather(WeatherIcon::Sun)),
            ..ClockContent::default()
        };
        let (number_size, _, unit_size, unit_right) = number_layout(&config, &content);
        assert_eq!(bottom_icon_x(&config), 568.0);
        assert_eq!(unit_right, 508.0);
        assert!(unit_size < number_size);
    }

    #[test]
    fn every_weather_mode_keeps_the_icon_inside_the_canvas() {
        for mode in [
            "weather",
            "weather-temp-c",
            "weather-temp-f",
            "weather-rain",
        ] {
            let mut config = AppConfig::default();
            config.bottom.mode = mode.into();
            assert_eq!(bottom_icon_x(&config), 568.0);
            assert!(bottom_icon_x(&config) + 43.0 < crate::config::BASE_WIDTH as f32);
        }
    }
}
