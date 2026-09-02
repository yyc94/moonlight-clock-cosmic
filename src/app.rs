use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use chrono::Local;
use cosmic::Element;
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::widget::{canvas, mouse_area};
use cosmic::iced::{Length, Subscription, window};

use crate::config::AppConfig;
use crate::content::build_content;
use crate::layer::{self, LayerEvent, LayerSurface};
use crate::paths::AppPaths;
use crate::platform::probe_wayland;
use crate::render::ClockCanvas;
use crate::weather::{
    WeatherError, WeatherErrorKind, WeatherSnapshot, fetch_and_cache, read_cache, unix_time,
};

pub fn run(paths: AppPaths) -> Result<()> {
    let capabilities = probe_wayland()?;
    if !capabilities.layer_shell {
        bail!("the Wayland compositor does not provide the wlr layer-shell protocol");
    }
    paths.ensure_config()?;
    let config = AppConfig::load(&paths.config_file)?;
    let settings = Settings::default()
        .antialiasing(true)
        .client_decorations(false)
        .debug(false)
        .scale_factor(1.0)
        .transparent(true)
        .no_main_window(true)
        .exit_on_close(false);
    cosmic::app::run::<ClockApp>(settings, Flags { paths, config })
        .context("libcosmic application failed")?;
    Ok(())
}

#[derive(Clone)]
pub struct Flags {
    paths: AppPaths,
    config: AppConfig,
}

pub struct ClockApp {
    core: Core,
    paths: AppPaths,
    config: AppConfig,
    config_mtime: Option<SystemTime>,
    refresh_mtime: Option<SystemTime>,
    weather: Option<WeatherSnapshot>,
    weather_error: Option<WeatherErrorKind>,
    weather_fetching: bool,
    last_weather_attempt: f64,
    layer_surface: LayerSurface,
}

#[derive(Clone, Debug)]
pub enum Message {
    Tick,
    Refresh,
    WeatherLoaded(Result<WeatherSnapshot, WeatherError>),
    Layer(LayerEvent),
}

impl cosmic::Application for ClockApp {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = "io.github.moonlight-clock";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        core.set_auto_corner_radius(enumflags2::BitFlags::empty());
        let config_mtime = modified(&flags.paths.config_file);
        let refresh_mtime = modified(&flags.paths.refresh_file);
        let weather = read_cache(&flags.paths.cache_file)
            .filter(|snapshot| snapshot.source_matches(&flags.config.weather));
        let layer_surface = LayerSurface::new(&flags.config.window);
        let mut app = Self {
            core,
            paths: flags.paths,
            config: flags.config,
            config_mtime,
            refresh_mtime,
            weather,
            weather_error: None,
            weather_fetching: false,
            last_weather_attempt: 0.0,
            layer_surface,
        };
        let surface = app.layer_surface.initial_task(&app.config);
        let weather = app.weather_task(false);
        (app, Task::batch([surface, weather]))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        unreachable!("Moonlight Clock does not create a main window")
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Self::Message> {
        let error = if self.config.uses_weather()
            && (!self.config.weather.enabled || self.config.weather.api_key.is_empty())
        {
            Some(WeatherErrorKind::NotConfigured)
        } else {
            self.weather_error
        };
        let content = build_content(
            &self.config,
            Local::now().fixed_offset(),
            self.weather.as_ref(),
            error,
        );
        let clock: Element<'_, Message> = canvas(ClockCanvas {
            config: self.config.clone(),
            content,
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
        mouse_area(clock).on_press(Message::Refresh).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let ticks = cosmic::iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick);
        let outputs = layer::subscription().map(Message::Layer);
        Subscription::batch([ticks, outputs])
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Tick => {
                let mut tasks = Vec::new();
                if modified(&self.paths.config_file) != self.config_mtime {
                    self.config_mtime = modified(&self.paths.config_file);
                    match AppConfig::load(&self.paths.config_file) {
                        Ok(config) => {
                            let window_changed = config.window != self.config.window;
                            let monitor_changed =
                                config.window.monitor != self.config.window.monitor;
                            let source_changed = config.weather != self.config.weather;
                            self.config = config;
                            if source_changed {
                                self.weather = self.weather.take().filter(|snapshot| {
                                    snapshot.source_matches(&self.config.weather)
                                });
                                self.weather_error = None;
                            }
                            if monitor_changed {
                                tasks.push(self.layer_surface.sync_target(&self.config));
                            } else if window_changed {
                                tasks.push(self.layer_surface.update(&self.config.window));
                            }
                        }
                        Err(error) => tracing::error!(%error, "configuration reload failed"),
                    }
                }
                let refresh_mtime = modified(&self.paths.refresh_file);
                let force = refresh_mtime != self.refresh_mtime;
                self.refresh_mtime = refresh_mtime;
                tasks.push(self.weather_task(force));
                Task::batch(tasks)
            }
            Message::Refresh => self.weather_task(true),
            Message::WeatherLoaded(result) => {
                self.weather_fetching = false;
                match result {
                    Ok(snapshot) => {
                        self.weather = Some(snapshot);
                        self.weather_error = None;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "weather update failed");
                        self.weather_error = Some(error.kind);
                    }
                }
                Task::none()
            }
            Message::Layer(event) => self.layer_surface.handle_event(event, &self.config),
        }
    }
}

impl ClockApp {
    fn weather_task(&mut self, force: bool) -> Task<Message> {
        if !self.config.uses_weather() {
            self.weather_error = None;
            return Task::none();
        }
        if !self.config.weather.enabled || self.config.weather.api_key.is_empty() {
            self.weather_error = Some(WeatherErrorKind::NotConfigured);
            return Task::none();
        }
        let now = unix_time();
        let due = self
            .weather
            .as_ref()
            .is_none_or(|snapshot| snapshot.is_due(&self.config.weather, now));
        if self.weather_fetching || (!force && !due) || now - self.last_weather_attempt < 20.0 {
            return Task::none();
        }
        self.weather_fetching = true;
        self.last_weather_attempt = now;
        let config = self.config.weather.clone();
        let cache_file = self.paths.cache_file.clone();
        Task::perform(
            async move { fetch_and_cache(config, &cache_file).await },
            |result| cosmic::action::app(Message::WeatherLoaded(result)),
        )
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}
