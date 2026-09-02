use cosmic::app::Task;
use cosmic::iced::event::wayland::{Event as WaylandEvent, OutputEvent};
use cosmic::iced::event::{self, PlatformSpecific};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, IcedOutput, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::wayland::commands::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface, set_anchor,
    set_exclusive_zone, set_keyboard_interactivity, set_layer, set_margin, set_size,
};
use cosmic::iced::{Event, Limits, Subscription, window};
use wayland_client::protocol::wl_output::WlOutput;

use crate::config::{AppConfig, WindowConfig};

pub struct LayerSurface {
    id: window::Id,
    created: bool,
    output: Option<WlOutput>,
    outputs: Vec<OutputRecord>,
}

#[derive(Clone, Debug)]
struct OutputRecord {
    output: WlOutput,
    names: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum LayerEvent {
    Output(OutputEvent, WlOutput),
}

pub fn subscription() -> Subscription<LayerEvent> {
    event::listen_raw(|event, _, _| match event {
        Event::PlatformSpecific(PlatformSpecific::Wayland(WaylandEvent::Output(event, output))) => {
            Some(LayerEvent::Output(event, output))
        }
        _ => None,
    })
}

impl LayerSurface {
    pub fn new(config: &WindowConfig) -> Self {
        Self {
            id: window::Id::unique(),
            created: monitor_is_active(&config.monitor),
            output: None,
            outputs: Vec::new(),
        }
    }

    pub fn initial_task<Message>(&self, config: &AppConfig) -> Task<Message> {
        if self.created {
            get_layer_surface(settings(self.id, config, IcedOutput::Active))
        } else {
            tracing::info!(
                monitor = %config.window.monitor,
                "waiting for the configured Wayland output"
            );
            Task::none()
        }
    }

    pub fn update<Message: 'static>(&self, config: &WindowConfig) -> Task<Message> {
        if !self.created {
            return Task::none();
        }
        update_settings(self.id, config)
    }

    pub fn handle_event<Message: 'static>(
        &mut self,
        event: LayerEvent,
        config: &AppConfig,
    ) -> Task<Message> {
        let LayerEvent::Output(event, output) = event;
        match event {
            OutputEvent::Created(info) => {
                self.upsert_output(
                    output,
                    info.map(|info| {
                        output_names(info.make, info.model, info.name, info.description)
                    })
                    .unwrap_or_default(),
                );
            }
            OutputEvent::InfoUpdate(info) => {
                self.upsert_output(
                    output,
                    output_names(info.make, info.model, info.name, info.description),
                );
            }
            OutputEvent::Removed => {
                self.outputs.retain(|item| item.output != output);
                if self.output.as_ref() == Some(&output) {
                    self.created = false;
                    self.output = None;
                    return Task::batch([destroy_layer_surface(self.id), self.sync_target(config)]);
                }
            }
        }
        self.sync_target(config)
    }

    pub fn sync_target<Message: 'static>(&mut self, config: &AppConfig) -> Task<Message> {
        let desired = if monitor_is_active(&config.window.monitor) {
            Some(None)
        } else {
            find_output(&self.outputs, &config.window.monitor).map(Some)
        };
        let Some(desired) = desired else {
            if self.created {
                self.created = false;
                self.output = None;
                return destroy_layer_surface(self.id);
            }
            return Task::none();
        };
        if self.created && self.output == desired {
            return Task::none();
        }

        let old_id = self.created.then_some(self.id);
        self.id = window::Id::unique();
        self.created = true;
        self.output = desired.clone();
        let output = desired.map_or(IcedOutput::Active, IcedOutput::Output);
        let create = get_layer_surface(settings(self.id, config, output));
        if let Some(old_id) = old_id {
            Task::batch([destroy_layer_surface(old_id), create])
        } else {
            create
        }
    }

    fn upsert_output(&mut self, output: WlOutput, names: Vec<String>) {
        if let Some(item) = self.outputs.iter_mut().find(|item| item.output == output) {
            item.names = names;
        } else {
            self.outputs.push(OutputRecord { output, names });
        }
    }
}

fn settings(id: window::Id, config: &AppConfig, output: IcedOutput) -> SctkLayerSurfaceSettings {
    let (width, height) = config.window_size();
    SctkLayerSurfaceSettings {
        id,
        layer: layer(&config.window.stacking),
        keyboard_interactivity: keyboard(&config.window.focusable),
        anchor: anchor(&config.window.anchor),
        output,
        namespace: "moonlight-clock".into(),
        margin: margin(&config.window),
        size: Some((Some(width), Some(height))),
        exclusive_zone: exclusive_zone(&config.window, width, height),
        size_limits: Limits::NONE
            .min_width(width as f32)
            .max_width(width as f32)
            .min_height(height as f32)
            .max_height(height as f32),
        ..SctkLayerSurfaceSettings::default()
    }
}

fn update_settings<Message: 'static>(id: window::Id, config: &WindowConfig) -> Task<Message> {
    let width = ((crate::config::BASE_WIDTH as f32 * config.scale).round() as u32).max(1);
    let height = ((crate::config::BASE_HEIGHT as f32 * config.scale).round() as u32).max(1);
    let margin = margin(config);
    Task::batch([
        set_size(id, Some(width), Some(height)),
        set_anchor(id, anchor(&config.anchor)),
        set_margin(id, margin.top, margin.right, margin.bottom, margin.left),
        set_exclusive_zone(id, exclusive_zone(config, width, height)),
        set_layer(id, layer(&config.stacking)),
        set_keyboard_interactivity(id, keyboard(&config.focusable)),
    ])
}

fn monitor_is_active(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "primary" | "active"
    )
}

fn find_output(outputs: &[OutputRecord], configured: &str) -> Option<WlOutput> {
    if let Ok(index) = configured.trim().parse::<usize>() {
        return outputs.get(index).map(|item| item.output.clone());
    }
    outputs
        .iter()
        .find(|item| monitor_name_matches(&item.names, configured))
        .map(|item| item.output.clone())
}

fn monitor_name_matches(names: &[String], configured: &str) -> bool {
    names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(configured.trim()))
}

fn output_names(
    make: String,
    model: String,
    name: Option<String>,
    description: Option<String>,
) -> Vec<String> {
    let make_model = format!("{make} {model}");
    let mut names = vec![make, model, make_model];
    names.extend(name);
    names.extend(description);
    names.retain(|value| !value.trim().is_empty());
    names.sort_unstable();
    names.dedup();
    names
}

fn anchor(value: &str) -> Anchor {
    let mut result = Anchor::empty();
    if value.contains("top") {
        result |= Anchor::TOP;
    }
    if value.contains("bottom") {
        result |= Anchor::BOTTOM;
    }
    if value.contains("left") {
        result |= Anchor::LEFT;
    }
    if value.contains("right") {
        result |= Anchor::RIGHT;
    }
    result
}

fn margin(config: &WindowConfig) -> IcedMargin {
    IcedMargin {
        top: if config.anchor.contains("top") {
            config.y
        } else {
            0
        },
        right: if config.anchor.contains("right") {
            config.x
        } else {
            0
        },
        bottom: if config.anchor.contains("bottom") {
            config.y
        } else {
            0
        },
        left: if config.anchor.contains("left") {
            config.x
        } else {
            0
        },
    }
}

fn layer(value: &str) -> Layer {
    match value {
        "background" => Layer::Background,
        "top" => Layer::Top,
        "overlay" => Layer::Overlay,
        _ => Layer::Bottom,
    }
}

fn keyboard(value: &str) -> KeyboardInteractivity {
    match value {
        "exclusive" => KeyboardInteractivity::Exclusive,
        "ondemand" => KeyboardInteractivity::OnDemand,
        _ => KeyboardInteractivity::None,
    }
}

fn exclusive_zone(config: &WindowConfig, width: u32, height: u32) -> i32 {
    if !config.exclusive {
        0
    } else if config.anchor.contains("top") || config.anchor.contains("bottom") {
        height.try_into().unwrap_or(i32::MAX)
    } else {
        width.try_into().unwrap_or(i32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_right_surface_maps_config_to_layer_shell() {
        let config = AppConfig::default();
        let settings = settings(window::Id::unique(), &config, IcedOutput::Active);
        assert!(settings.anchor.contains(Anchor::TOP));
        assert!(settings.anchor.contains(Anchor::RIGHT));
        assert_eq!(settings.margin.right, 0);
        assert_eq!(settings.size, Some((Some(614), Some(387))));
        assert_eq!(settings.exclusive_zone, 0);
    }

    #[test]
    fn scaled_exclusive_surface_reserves_its_height() {
        let mut config = AppConfig::default();
        config.window.scale = 1.5;
        config.window.exclusive = true;
        let settings = settings(window::Id::unique(), &config, IcedOutput::Active);
        assert_eq!(settings.size, Some((Some(921), Some(581))));
        assert_eq!(settings.exclusive_zone, 581);
    }

    #[test]
    fn monitor_names_are_trimmed_and_case_insensitive() {
        let names = output_names("Dell".into(), "U2723QE".into(), Some("DP-1".into()), None);
        assert!(monitor_name_matches(&names, " dp-1 "));
        assert!(monitor_name_matches(&names, "DELL U2723QE"));
        assert!(!monitor_name_matches(&names, "HDMI-A-1"));
        assert!(monitor_is_active("PRIMARY"));
    }
}
