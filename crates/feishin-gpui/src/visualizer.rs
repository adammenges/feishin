use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use anyhow::Context as _;
use feishin_core::visualizer::{self, Config, enabled, number};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Colorize, Disableable, StyledExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    color_picker::{ColorPicker, ColorPickerState},
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    menu::{DropdownMenu, PopupMenuItem},
};
use raw_window_handle::HasWindowHandle;
use serde_json::{Value, json};

pub const VISUALIZER_NUMBERS: &[(bool, &str, &str, f32, f32, f32)] = &[
    (true, "blendTime", "Blend duration (seconds)", 0., 60., 0.5),
    (true, "cycleTime", "Cycle duration (seconds)", 1., 3600., 1.),
    (
        true,
        "maxFPS",
        "Maximum FPS (0 = display rate)",
        0.,
        240.,
        1.,
    ),
    (true, "opacity", "Opacity", 0., 1., 0.05),
    (false, "minFreq", "Minimum frequency (Hz)", 1., 96000., 1.),
    (false, "maxFreq", "Maximum frequency (Hz)", 1., 96000., 1.),
    (false, "minDecibels", "Minimum dB", -200., 0., 1.),
    (false, "maxDecibels", "Maximum dB", -200., 50., 1.),
    (false, "smoothing", "Smoothing", 0., 1., 0.05),
    (false, "opacity", "Opacity", 0., 1., 0.05),
    (false, "barSpace", "Bar spacing", 0., 100., 0.05),
    (false, "lineWidth", "Line width", 0., 100., 0.1),
    (false, "fillAlpha", "Graph fill opacity", 0., 1., 0.05),
    (false, "radius", "Radial radius", 0., 1., 0.05),
    (false, "spinSpeed", "Radial spin", -100., 100., 0.1),
    (false, "reflexRatio", "Reflection height", 0., 0.99, 0.05),
    (false, "reflexAlpha", "Reflection opacity", 0., 1., 0.05),
    (false, "reflexBright", "Reflection brightness", 0., 10., 0.1),
    (false, "peakHoldTime", "Peak hold (ms)", 0., 60000., 50.),
    (false, "peakFadeTime", "Peak fade (ms)", 0., 60000., 50.),
    (false, "gravity", "Peak gravity", 0., 100., 0.1),
    (false, "volume", "Analyzer gain", 0., 10., 0.1),
    (
        false,
        "linearBoost",
        "Linear amplitude boost",
        0.,
        100.,
        0.1,
    ),
    (
        false,
        "maxFPS",
        "Maximum FPS (0 = display rate)",
        0.,
        240.,
        1.,
    ),
];
const SPECTRUM_FLAGS: &[(&str, &str)] = &[
    ("alphaBars", "Bar alpha follows level"),
    ("ansiBands", "ANSI octave bands"),
    ("fadePeaks", "Fade peaks"),
    ("ledBars", "LED bars"),
    ("linearAmplitude", "Linear amplitude"),
    ("loRes", "Low resolution"),
    ("lumiBars", "Full-height luminance bars"),
    ("noteLabels", "Musical note labels"),
    ("outlineBars", "Outline bars"),
    ("peakLine", "Peak line"),
    ("radial", "Radial"),
    ("radialInvert", "Inverted radial bars"),
    ("reflexFit", "Fit reflection"),
    ("roundBars", "Round bars"),
    ("showFPS", "Show FPS"),
    ("showPeaks", "Show peaks"),
    ("showScaleX", "Frequency scale"),
    ("showScaleY", "Level scale"),
    ("splitGradient", "Split gradient"),
    ("trueLeds", "LED color levels"),
];
const BUILTIN_GRADIENTS: &[&str] = &["classic", "prism", "rainbow", "steelblue", "orangered"];
pub struct GradientEditor {
    pub original_name: Option<String>,
    pub name: Entity<InputState>,
    pub horizontal: bool,
    pub stops: Vec<GradientStop>,
    pub _subscriptions: Vec<Subscription>,
}
pub struct GradientStop {
    pub color: Entity<ColorPickerState>,
    pub position: Entity<InputState>,
    pub level: Entity<InputState>,
    pub position_enabled: bool,
    pub level_enabled: bool,
}

impl Feishin {
    pub fn system_audio_blocked(&self) -> bool {
        let value = self.state.settings.mpv_properties.get("audio-exclusive");
        value.is_some_and(|value| value.as_bool() == Some(true) || value.as_str() == Some("yes"))
    }
    pub fn sync_system_audio(&mut self, visible: bool) {
        let needed = visible
            && !self.state.settings.mpv_path.is_empty()
            && self.cast_selected.is_none()
            && self.state.settings.visualizer.system_audio_consent
            && !self.system_audio_blocked();
        if !needed {
            self.system_audio = None;
            self.system_audio_ready = false;
            self.system_audio_error = None;
            return;
        }
        if self.system_audio.is_none() && self.system_audio_error.is_none() {
            self.system_audio = Some(feishin_core::system_audio::SystemAudio::start(
                self.state.settings.ffmpeg_path.clone(),
            ));
        }
        let events = self
            .system_audio
            .as_ref()
            .map(|audio| audio.events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                feishin_core::system_audio::Event::Started => {
                    self.system_audio_ready =
                        self.system_audio.is_some() && self.system_audio_error.is_none();
                }
                feishin_core::system_audio::Event::Error(error) => {
                    self.system_audio_error = Some(error);
                    self.system_audio = None;
                    self.system_audio_ready = false;
                }
            }
        }
    }
    pub fn visualizer_capture(&self) -> feishin_core::visualizer::Capture {
        self.system_audio
            .as_ref()
            .map(|audio| audio.capture.clone())
            .unwrap_or_else(|| self.player.capture.clone())
    }
    pub fn visualizer_has_audio(&self) -> bool {
        self.system_audio_ready
            || (self.playing
                && self.state.settings.mpv_path.is_empty()
                && self.cast_selected.is_none())
    }
    pub fn system_audio_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let consent = self.state.settings.visualizer.system_audio_consent;
        let blocked = self.system_audio_blocked();
        div().v_flex().gap_2()
            .when(!self.state.settings.mpv_path.is_empty() && self.cast_selected.is_none(), |div| {
                div.child(crate::styles::button("system-audio-capture", cx)
                    .label(if consent { "Stop system-audio capture" } else { "Allow system-audio capture" })
                    .disabled(blocked)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.visualizer.system_audio_consent = !this.state.settings.visualizer.system_audio_consent;
                        this.system_audio_error = None;
                        this.save(); cx.notify();
                    })))
                    .child(if blocked { "System-audio capture is unavailable with exclusive audio output." }
                        else if self.system_audio_ready { "Capturing system audio for the visualizer." }
                        else if consent && self.system_audio_error.is_none() { "Connecting system audio..." }
                        else { "Allow capture to visualize MPV playback. Capture includes audio from other apps and may request OS permission." })
            })
            .when_some(self.system_audio_error.clone(), |div, error| {
                div.child(error).child(crate::styles::button("system-audio-retry", cx).label(crate::appearance::text("Retry capture", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.system_audio_error = None; cx.notify();
                    })))
            }).into_any_element()
    }
    pub fn edit_visualizer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Visualizer configuration")
                .multi_line(true)
        });
        input.update(cx, |input, cx| {
            input.set_value(
                serde_json::to_string_pretty(&self.state.settings.visualizer).unwrap_or_default(),
                window,
                cx,
            )
        });
        self.visualizer_settings = Some(input);
        self.visualizer_gradient_editor = None;
        self.init_visualizer_numbers(window, cx);
        self.form_name
            .update(cx, |input, cx| input.set_value("", window, cx));
        if self.visualizer_display.mounted {
            self.open_visualizer_settings_dialog(window, cx);
        } else {
            self.navigate(Route::VisualizerSettings, cx);
        }
    }
    pub fn apply_visualizer(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.visualizer_settings else {
            return;
        };
        let result = (|| -> anyhow::Result<Config> {
            let config = serde_json::from_str(input.read(cx).value().as_ref())?;
            visualizer::validate(&config)?;
            Ok(config)
        })();
        match result {
            Ok(config) => {
                if self.message.starts_with("Invalid visualizer settings:") {
                    self.message.clear();
                }
                self.state.settings.visualizer = config;
                self.milkdrop_error = None;
                self.milkdrop_cycle_elapsed = 0.;
                self.save();
                if self.visualizer_display.settings_open {
                    self.visualizer_display.settings_close_requested = true;
                    cx.notify();
                } else {
                    self.back(cx);
                }
            }
            Err(error) => {
                self.message = format!("Invalid visualizer settings: {error}");
                cx.notify();
            }
        }
    }
    fn init_visualizer_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visualizer_subscriptions.clear();
        self.visualizer_numbers = VISUALIZER_NUMBERS
            .iter()
            .map(|(_, _, label, _, _, _)| {
                cx.new(|cx| crate::appearance::input_state(window, cx).placeholder(*label))
            })
            .collect();
        self.sync_visualizer_numbers(window, cx);
        for (index, input) in self.visualizer_numbers.clone().iter().enumerate() {
            self.visualizer_subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        this.apply_visualizer_number(index, window, cx);
                    }
                },
            ));
            self.visualizer_subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, _, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let (_, _, _, min, max, step) = VISUALIZER_NUMBERS[index];
                    let input = this.visualizer_numbers[index].clone();
                    let value = input.read(cx).value().parse::<f32>().unwrap_or(min);
                    let value = (value
                        + if *action == StepAction::Increment {
                            step
                        } else {
                            -step
                        })
                    .clamp(min, max);
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                    this.apply_visualizer_number(index, window, cx);
                },
            ));
        }
        if let Some(input) = &self.visualizer_settings {
            self.visualizer_subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur) {
                        this.sync_visualizer_numbers(window, cx);
                    }
                    cx.notify();
                },
            ));
        }
    }
    fn visualizer_draft(&self, cx: &App) -> anyhow::Result<Config> {
        serde_json::from_str(
            self.visualizer_settings
                .as_ref()
                .context("Visualizer settings are closed")?
                .read(cx)
                .value()
                .as_ref(),
        )
        .map_err(Into::into)
    }
    fn set_visualizer_draft(
        &mut self,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = &self.visualizer_settings {
            input.update(cx, |input, cx| {
                input.set_value(serde_json::to_string_pretty(config).unwrap(), window, cx)
            });
        }
        cx.notify();
    }
    fn sync_visualizer_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(config) = self.visualizer_draft(cx) else {
            return;
        };
        let defaults = Config::default();
        for (index, input) in self.visualizer_numbers.iter().enumerate() {
            let (milk, key, _, _, _, _) = VISUALIZER_NUMBERS[index];
            let (options, fallback) = if milk {
                (&config.butterchurn, &defaults.butterchurn)
            } else {
                (&config.audiomotionanalyzer, &defaults.audiomotionanalyzer)
            };
            let value = options[key]
                .as_f64()
                .or_else(|| fallback[key].as_f64())
                .unwrap_or_default();
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
    }
    pub fn apply_visualizer_number(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = self.visualizer_numbers.get(index) else {
            return;
        };
        let (milk, key, label, min, max, _) = VISUALIZER_NUMBERS[index];
        let value = input.read(cx).value().parse::<f32>();
        match value {
            Ok(value) if value.is_finite() && (min..=max).contains(&value) => {
                self.edit_visualizer_option(milk, key, json!(value), window, cx)
            }
            _ => {
                self.message = format!("{label} must be between {min} and {max}");
                cx.notify();
            }
        }
    }
    fn edit_visualizer_option(
        &mut self,
        milk: bool,
        key: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.visualizer_draft(cx) {
            Ok(mut config) => {
                let options = if milk {
                    &mut config.butterchurn
                } else {
                    &mut config.audiomotionanalyzer
                };
                options[key] = value;
                self.set_visualizer_draft(&config, window, cx);
            }
            Err(error) => {
                self.message = format!("Invalid advanced configuration: {error}");
                cx.notify();
            }
        }
    }
    pub fn save_visualizer_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<Config> {
            let name = self.form_name.read(cx).value().trim().to_owned();
            anyhow::ensure!(!name.is_empty() && name.len() <= 256, "Enter a preset name");
            let mut config = self.visualizer_draft(cx)?;
            visualizer::validate(&config)?;
            let mut preset = config.audiomotionanalyzer.clone();
            preset.as_object_mut().unwrap().remove("presets");
            preset.as_object_mut().unwrap().remove("customGradients");
            let mut presets = config.audiomotionanalyzer["presets"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            presets.push(
                json!({"id": uuid::Uuid::new_v4().to_string(), "name": name, "value": preset}),
            );
            config.audiomotionanalyzer["presets"] = json!(presets);
            visualizer::validate(&config)?;
            Ok(config)
        })();
        match result {
            Ok(config) => self.set_visualizer_draft(&config, window, cx),
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
            }
        }
    }
    pub fn choose_spectrum_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        let options = &mut self.state.settings.visualizer.audiomotionanalyzer;
        let Some(preset) = options["presets"][index]["value"].as_object().cloned() else {
            return;
        };
        if let Some(options) = options.as_object_mut() {
            options.extend(preset);
        }
        self.save();
        cx.notify();
    }
    pub fn edit_visualizer_gradient(
        &mut self,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(config) = self.visualizer_draft(cx) else {
            return;
        };
        let gradient = config.audiomotionanalyzer["customGradients"]
            .as_array()
            .and_then(|gradients| {
                gradients
                    .iter()
                    .find(|g| g["name"].as_str() == name.as_deref())
            })
            .cloned()
            .unwrap_or_else(|| {
                json!({"name":"", "dir":"v", "colorStops":[
                    {"color":"#ff0000"}, {"color":"#0000ff"}
                ]})
            });
        let name_input =
            cx.new(|cx| crate::appearance::input_state(window, cx).placeholder("Gradient name"));
        name_input.update(cx, |input, cx| {
            input.set_value(
                gradient["name"].as_str().unwrap_or_default().to_owned(),
                window,
                cx,
            )
        });
        let mut editor = GradientEditor {
            original_name: name,
            name: name_input,
            horizontal: gradient["dir"].as_str() == Some("h"),
            stops: Vec::new(),
            _subscriptions: Vec::new(),
        };
        for stop in gradient["colorStops"].as_array().into_iter().flatten() {
            let (stop, subscriptions) = gradient_stop(stop, window, cx);
            editor.stops.push(stop);
            editor._subscriptions.extend(subscriptions);
        }
        self.visualizer_gradient_editor = Some(editor);
        cx.notify();
    }
    pub fn save_visualizer_gradient(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<Config> {
            let editor = self
                .visualizer_gradient_editor
                .as_ref()
                .context("Gradient editor is closed")?;
            let name = editor.name.read(cx).value().trim().to_owned();
            anyhow::ensure!(
                !name.is_empty() && name.len() <= 128,
                "Enter a gradient name of at most 128 bytes"
            );
            anyhow::ensure!(
                !BUILTIN_GRADIENTS.contains(&name.as_str()),
                "Choose a name other than a built-in gradient"
            );
            let mut config = self.visualizer_draft(cx)?;
            let mut gradients = config.audiomotionanalyzer["customGradients"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            anyhow::ensure!(
                !gradients.iter().any(|g| g["name"].as_str() == Some(&name)
                    && Some(name.as_str()) != editor.original_name.as_deref()),
                "A gradient with that name already exists"
            );
            let stops = editor.stops.iter().map(|stop| -> anyhow::Result<Value> {
                let color = stop.color.read(cx).value().context("Choose a color for each stop")?;
                let mut value = json!({"color": color.to_hex(), "positionEnabled": stop.position_enabled, "levelEnabled": stop.level_enabled});
                for (enabled, key, input) in [
                    (stop.position_enabled, "pos", &stop.position),
                    (stop.level_enabled, "level", &stop.level),
                ] {
                    if enabled {
                        let number = input.read(cx).value().parse::<f32>().context("Stop position and level must be numbers between 0 and 1")?;
                        anyhow::ensure!(number.is_finite() && (0. ..=1.).contains(&number), "Stop position and level must be between 0 and 1");
                        value[key] = json!(number);
                    }
                }
                Ok(value)
            }).collect::<anyhow::Result<Vec<_>>>()?;
            let gradient = json!({"name":name, "dir": if editor.horizontal {"h"} else {"v"}, "colorStops":stops});
            if let Some(index) = gradients
                .iter()
                .position(|g| g["name"].as_str() == editor.original_name.as_deref())
            {
                gradients[index] = gradient;
            } else {
                gradients.push(gradient);
            }
            for key in ["gradient", "gradientLeft", "gradientRight"] {
                if editor.original_name.is_some()
                    && config.audiomotionanalyzer[key].as_str() == editor.original_name.as_deref()
                {
                    config.audiomotionanalyzer[key] = json!(name);
                }
            }
            config.audiomotionanalyzer["customGradients"] = json!(gradients);
            visualizer::validate(&config)?;
            Ok(config)
        })();
        match result {
            Ok(config) => {
                self.set_visualizer_draft(&config, window, cx);
                self.visualizer_gradient_editor = None;
            }
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }
    pub fn delete_visualizer_gradient(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Ok(mut config) = self.visualizer_draft(cx) {
            if let Some(gradients) = config.audiomotionanalyzer["customGradients"].as_array_mut() {
                gradients.retain(|gradient| gradient["name"].as_str() != Some(name));
            }
            for key in ["gradient", "gradientLeft", "gradientRight"] {
                if config.audiomotionanalyzer[key].as_str() == Some(name) {
                    config.audiomotionanalyzer[key] = json!("prism");
                }
            }
            if self
                .visualizer_gradient_editor
                .as_ref()
                .is_some_and(|editor| editor.original_name.as_deref() == Some(name))
            {
                self.visualizer_gradient_editor = None;
            }
            self.set_visualizer_draft(&config, window, cx);
        }
    }
    pub fn visualizer_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(input) = &self.visualizer_settings else {
            return div().into_any_element();
        };
        let config = self
            .visualizer_draft(cx)
            .unwrap_or_else(|_| self.state.settings.visualizer.clone());
        let milk = config.kind == "butterchurn";
        let options = if milk {
            &config.butterchurn
        } else {
            &config.audiomotionanalyzer
        };
        div()
            .id("visualizer-settings")
            .debug_selector(|| "visualizer-settings".into())
            .v_flex()
            .gap_3()
            .h_full()
            .native_vertical_scroll()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .child(
                        crate::styles::button("save-visualizer-settings", cx)
                            .debug_selector(|| "save-visualizer-settings".into())
                            .label(crate::appearance::text("Apply", cx))
                            .on_click(cx.listener(|this, _, _, cx| this.apply_visualizer(cx))),
                    )
                    .child(
                        crate::styles::button("reset-visualizer-settings", cx)
                            .label(crate::appearance::text("Reset", cx))
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(input) = &this.visualizer_settings {
                                    input.update(cx, |input, cx| {
                                        input.set_value(
                                            serde_json::to_string_pretty(&Config::default())
                                                .unwrap(),
                                            window,
                                            cx,
                                        )
                                    });
                                }
                                this.sync_visualizer_numbers(window, cx);
                            })),
                    ),
            )
            .child(
                crate::styles::button("visualizer-kind", cx)
                    .label(if milk {
                        "MilkDrop"
                    } else {
                        "Spectrum analyzer"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Ok(mut config) = this.visualizer_draft(cx) {
                            config.kind = if config.kind == "butterchurn" {
                                "audiomotionanalyzer"
                            } else {
                                "butterchurn"
                            }
                            .into();
                            this.set_visualizer_draft(&config, window, cx);
                        }
                    })),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_3()
                    .children(
                        self.visualizer_numbers
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| VISUALIZER_NUMBERS[*index].0 == milk)
                            .map(|(index, input)| {
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .w_56()
                                    .child(VISUALIZER_NUMBERS[index].2)
                                    .debug_selector(move || format!("visualizer-number-{index}"))
                                    .child(crate::styles::number_input(input, cx))
                            }),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_3()
                    .children(
                        if milk {
                            &[
                                ("cyclePresets", "Cycle presets"),
                                ("randomizeNextPreset", "Random cycling"),
                                ("includeAllPresets", "Cycle all presets"),
                            ][..]
                        } else {
                            SPECTRUM_FLAGS
                        }
                        .iter()
                        .map(|&(key, label)| {
                            Checkbox::new(key)
                                .label(label)
                                .checked(enabled(options, key))
                                .on_click(cx.listener(move |this, value: &bool, window, cx| {
                                    this.edit_visualizer_option(milk, key, json!(value), window, cx)
                                }))
                        }),
                    ),
            )
            .when(!milk, |container| {
                let enums: &[(&str, &str, &[&str])] = &[
                    (
                        "mode",
                        "Mode",
                        &["0", "1", "2", "3", "4", "5", "6", "7", "8", "10"],
                    ),
                    (
                        "fftSize",
                        "FFT size",
                        &[
                            "32", "64", "128", "256", "512", "1024", "2048", "4096", "8192",
                            "16384", "32768",
                        ],
                    ),
                    (
                        "channelLayout",
                        "Channels",
                        &[
                            "single",
                            "dual-horizontal",
                            "dual-vertical",
                            "dual-combined",
                        ],
                    ),
                    (
                        "frequencyScale",
                        "Frequency spacing",
                        &["log", "linear", "bark", "mel"],
                    ),
                    (
                        "colorMode",
                        "Color",
                        &["gradient", "bar-index", "bar-level"],
                    ),
                    (
                        "weightingFilter",
                        "Weighting",
                        &["", "A", "B", "C", "D", "Z", "468"],
                    ),
                    ("mirror", "Mirror", &["-1", "0", "1"]),
                ];
                container
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .flex_wrap()
                            .gap_2()
                            .children(enums.iter().map(|&(key, label, values)| {
                                crate::styles::button(key, cx)
                                    .label(format!(
                                        "{label}: {}",
                                        options[key]
                                            .as_str()
                                            .map(str::to_owned)
                                            .unwrap_or_else(|| options[key].to_string())
                                    ))
                                    .dropdown_menu({
                                        let entity = cx.entity();
                                        move |mut menu, _, _| {
                                            for &value in values {
                                                let entity = entity.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(if value.is_empty() {
                                                        "None"
                                                    } else {
                                                        value
                                                    })
                                                    .on_click(move |_, window, cx| {
                                                        entity.update(cx, |this, cx| {
                                                            this.edit_visualizer_option(
                                                                false,
                                                                key,
                                                                if ["mode", "fftSize", "mirror"]
                                                                    .contains(&key)
                                                                {
                                                                    json!(
                                                                        value
                                                                            .parse::<i32>()
                                                                            .unwrap()
                                                                    )
                                                                } else {
                                                                    json!(value)
                                                                },
                                                                window,
                                                                cx,
                                                            )
                                                        });
                                                    }),
                                                );
                                            }
                                            menu
                                        }
                                    })
                            })),
                    )
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .child(crate::styles::input(&self.form_name, cx))
                            .child(
                                crate::styles::button("save-spectrum-preset", cx)
                                    .label(crate::appearance::text(
                                        "Save current spectrum as preset",
                                        cx,
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_visualizer_preset(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .flex_wrap()
                            .gap_2()
                            .children(
                                options["presets"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .enumerate()
                                    .map(|(index, preset)| {
                                        div()
                                            .h_flex()
                                            .when(crate::appearance::rtl(cx), |div| {
                                                div.flex_row_reverse()
                                            })
                                            .gap_1()
                                            .child(
                                                preset["name"]
                                                    .as_str()
                                                    .unwrap_or("Preset")
                                                    .to_owned(),
                                            )
                                            .child(
                                                crate::styles::button(
                                                    ("delete-spectrum-preset", index),
                                                    cx,
                                                )
                                                .ghost()
                                                .label(crate::appearance::text("Remove", cx))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        if let Ok(mut config) =
                                                            this.visualizer_draft(cx)
                                                            && let Some(presets) = config
                                                                .audiomotionanalyzer["presets"]
                                                                .as_array_mut()
                                                            && index < presets.len()
                                                        {
                                                            presets.remove(index);
                                                            this.set_visualizer_draft(
                                                                &config, window, cx,
                                                            );
                                                        }
                                                    },
                                                )),
                                            )
                                    }),
                            ),
                    )
            })
            .when(!milk, |container| {
                container.child(self.visualizer_gradients_view(&config, cx))
            })
            .child(crate::appearance::text(
                "Advanced configuration (all preset and gradient properties)",
                cx,
            ))
            .child(crate::styles::input(input, cx).h(px(300.0)).flex_shrink_0())
            .into_any_element()
    }
    fn visualizer_gradients_view(&self, config: &Config, cx: &mut Context<Self>) -> AnyElement {
        let options = &config.audiomotionanalyzer;
        let gradients = options["customGradients"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let names: Vec<String> = BUILTIN_GRADIENTS
            .iter()
            .map(|name| (*name).to_owned())
            .chain(
                gradients
                    .iter()
                    .filter_map(|g| g["name"].as_str().map(str::to_owned)),
            )
            .collect();
        div()
            .v_flex()
            .gap_2()
            .child(crate::appearance::text("Gradients", cx))
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .children(
                        [
                            ("gradient", "Single channel"),
                            ("gradientLeft", "Left channel"),
                            ("gradientRight", "Right channel"),
                        ]
                        .into_iter()
                        .map(|(key, label)| {
                            crate::styles::button(key, cx)
                                .label(format!(
                                    "{label}: {}",
                                    options[key].as_str().unwrap_or("prism")
                                ))
                                .dropdown_menu({
                                    let names = names.clone();
                                    let entity = cx.entity();
                                    move |mut menu, _, _| {
                                        for name in &names {
                                            let name = name.clone();
                                            let entity = entity.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(name.clone()).on_click(
                                                    move |_, window, cx| {
                                                        let name = name.clone();
                                                        entity.update(cx, |this, cx| {
                                                            this.edit_visualizer_option(
                                                                false,
                                                                key,
                                                                json!(name),
                                                                window,
                                                                cx,
                                                            )
                                                        });
                                                    },
                                                ),
                                            );
                                        }
                                        menu
                                    }
                                })
                        }),
                    ),
            )
            .child(
                crate::styles::button("new-visualizer-gradient", cx)
                    .label(crate::appearance::text("New gradient", cx))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit_visualizer_gradient(None, window, cx)
                    })),
            )
            .children(gradients.iter().enumerate().map(|(index, gradient)| {
                let name = gradient["name"].as_str().unwrap_or_default().to_owned();
                let delete_name = name.clone();
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .child(name.clone())
                    .child(
                        crate::styles::button(("edit-visualizer-gradient", index), cx)
                            .label(crate::appearance::text("Edit", cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_visualizer_gradient(Some(name.clone()), window, cx)
                            })),
                    )
                    .child(
                        crate::styles::button(("delete-visualizer-gradient", index), cx)
                            .ghost()
                            .label(crate::appearance::text("Remove", cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.delete_visualizer_gradient(&delete_name, window, cx)
                            })),
                    )
            }))
            .when_some(
                self.visualizer_gradient_editor.as_ref(),
                |container, editor| {
                    container.child(
                        div()
                            .id("visualizer-gradient-editor")
                            .debug_selector(|| "visualizer-gradient-editor".into())
                            .v_flex()
                            .gap_2()
                            .child(crate::styles::input(&editor.name, cx))
                            .child(
                                Checkbox::new("gradient-horizontal")
                                    .label(crate::appearance::text("Horizontal", cx))
                                    .checked(editor.horizontal)
                                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                                        if let Some(editor) = &mut this.visualizer_gradient_editor {
                                            editor.horizontal = *value;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .children(editor.stops.iter().enumerate().map(|(index, stop)| {
                                div()
                                    .h_flex()
                                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                    .gap_2()
                                    .flex_wrap()
                                    .child(ColorPicker::new(&stop.color))
                                    .child(
                                        Checkbox::new(("gradient-position-enabled", index))
                                            .label(crate::appearance::text("Position", cx))
                                            .checked(stop.position_enabled)
                                            .on_click(cx.listener(
                                                move |this, value: &bool, _, cx| {
                                                    if let Some(stop) = this
                                                        .visualizer_gradient_editor
                                                        .as_mut()
                                                        .and_then(|e| e.stops.get_mut(index))
                                                    {
                                                        stop.position_enabled = *value;
                                                    }
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        crate::styles::number_input(&stop.position, cx)
                                            .disabled(!stop.position_enabled)
                                            .w_24(),
                                    )
                                    .child(
                                        Checkbox::new(("gradient-level-enabled", index))
                                            .label(crate::appearance::text("Level", cx))
                                            .checked(stop.level_enabled)
                                            .on_click(cx.listener(
                                                move |this, value: &bool, _, cx| {
                                                    if let Some(stop) = this
                                                        .visualizer_gradient_editor
                                                        .as_mut()
                                                        .and_then(|e| e.stops.get_mut(index))
                                                    {
                                                        stop.level_enabled = *value;
                                                    }
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        crate::styles::number_input(&stop.level, cx)
                                            .disabled(!stop.level_enabled)
                                            .w_24(),
                                    )
                                    .child(
                                        crate::styles::button(("remove-gradient-stop", index), cx)
                                            .ghost()
                                            .label(crate::appearance::text("Remove stop", cx))
                                            .disabled(editor.stops.len() == 1)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(editor) =
                                                    &mut this.visualizer_gradient_editor
                                                    && editor.stops.len() > 1
                                                    && index < editor.stops.len()
                                                {
                                                    editor.stops.remove(index);
                                                }
                                                cx.notify();
                                            })),
                                    )
                            }))
                            .child(
                                div()
                                    .h_flex()
                                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                    .gap_2()
                                    .child(
                                        crate::styles::button("add-gradient-stop", cx)
                                            .label(crate::appearance::text("Add stop", cx))
                                            .disabled(editor.stops.len() >= 100)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                let (stop, subscriptions) = gradient_stop(
                                                    &json!({"color":"#ffffff"}),
                                                    window,
                                                    cx,
                                                );
                                                if let Some(editor) =
                                                    &mut this.visualizer_gradient_editor
                                                    && editor.stops.len() < 100
                                                {
                                                    editor.stops.push(stop);
                                                    editor._subscriptions.extend(subscriptions);
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        crate::styles::button("save-visualizer-gradient", cx)
                                            .label(crate::appearance::text("Save gradient", cx))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_visualizer_gradient(window, cx)
                                            })),
                                    )
                                    .child(
                                        crate::styles::button("cancel-visualizer-gradient", cx)
                                            .ghost()
                                            .label(crate::appearance::text("Cancel", cx))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.visualizer_gradient_editor = None;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                },
            )
            .into_any_element()
    }
    pub fn visualizer_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.visualizer_surface(false, true, window, cx)
    }
    pub fn visualizer_surface(
        &mut self,
        compact: bool,
        interactive: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = &self.state.settings.visualizer;
        if config.kind == "butterchurn" {
            return self.milkdrop_view(compact, interactive, window, cx);
        }
        let elapsed = self.visualizer_frame_at.elapsed().as_secs_f32().min(1.);
        let max_fps = number(&config.audiomotionanalyzer, "maxFPS", 0.);
        if max_fps == 0. || elapsed >= 1. / max_fps {
            self.visualizer_fps = self.visualizer_fps * 0.8 + 0.2 / elapsed.max(0.001);
            let frames = self.visualizer_capture().snapshot(number(
                &config.audiomotionanalyzer,
                "fftSize",
                16384.,
            ) as usize);
            let silent = !self.visualizer_has_audio();
            self.visualizer_analyzer.dimensions = self.visualizer_dimensions.get();
            self.visualizer_analyzer.pixel_ratio = window.scale_factor();
            self.visualizer_analyzer.update(
                if silent { &[] } else { &frames },
                48000,
                &config.audiomotionanalyzer,
                elapsed,
            );
            self.visualizer_spin = visualizer::spin_angle(
                self.visualizer_spin,
                number(&config.audiomotionanalyzer, "spinSpeed", 0.),
                self.visualizer_analyzer
                    .bands
                    .iter()
                    .any(|band| band.values.iter().any(|value| *value > 0.)),
                elapsed,
            );
            self.visualizer_frame_at = std::time::Instant::now();
        }
        if self.playing || self.system_audio.is_some() {
            window.request_animation_frame();
        }
        let bands = self.visualizer_analyzer.bands.clone();
        let mut options = config.audiomotionanalyzer.clone();
        options["_spin"] = json!(self.visualizer_spin);
        options["_fps"] = json!(self.visualizer_fps);
        options["_bandCount"] = json!(bands.len());
        options["_pixelRatio"] =
            json!(window.scale_factor() * if enabled(&options, "loRes") { 0.5 } else { 1. });
        let accent = cx.theme().primary;
        let dimensions = self.visualizer_dimensions.clone();
        if compact {
            let surface = div()
                .size_full()
                .child(
                    canvas(
                        move |bounds, _, _| {
                            dimensions
                                .set([f32::from(bounds.size.width), f32::from(bounds.size.height)])
                        },
                        move |bounds, _, window, cx| {
                            paint_spectrum(bounds, &bands, &options, accent, window, cx)
                        },
                    )
                    .size_full(),
                )
                .into_any_element();
            return if interactive {
                self.visualizer_compact_frame(surface, window, cx)
            } else {
                surface
            };
        }
        let presets = options["presets"].as_array().cloned().unwrap_or_default();
        div()
            .id("native-visualizer")
            .debug_selector(|| "native-visualizer".into())
            .size_full()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_2()
                    .child(
                        crate::styles::button("visualizer-configure", cx)
                            .label(crate::appearance::text("Settings", cx))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.edit_visualizer(window, cx)),
                            ),
                    )
                    .child(
                        crate::styles::button("visualizer-fullscreen", cx)
                            .label(self.tr("common.maximize"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_expanded_visualizer(window, cx)
                            })),
                    )
                    .child(
                        crate::styles::button("switch-milkdrop", cx)
                            .label(crate::appearance::text("MilkDrop", cx))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.settings.visualizer.kind = "butterchurn".into();
                                this.milkdrop_error = None;
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button("visualizer-mode", cx)
                            .label(format!("Mode {}", number(&options, "mode", 10.)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let v = &mut this.state.settings.visualizer.audiomotionanalyzer;
                                v["mode"] = json!(match v["mode"].as_u64().unwrap_or(10) {
                                    0..=7 => v["mode"].as_u64().unwrap() + 1,
                                    8 => 10,
                                    _ => 0,
                                });
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button("visualizer-radial", cx)
                            .label(if enabled(&options, "radial") {
                                "Radial"
                            } else {
                                "Horizontal"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                let v = &mut this.state.settings.visualizer.audiomotionanalyzer;
                                v["radial"] = json!(!enabled(v, "radial"));
                                this.save();
                                cx.notify();
                            })),
                    )
                    .children(presets.iter().enumerate().map(|(index, preset)| {
                        crate::styles::button(("spectrum-preset", index), cx)
                            .ghost()
                            .label(preset["name"].as_str().unwrap_or("Preset").to_owned())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose_spectrum_preset(index, cx)
                            }))
                    })),
            )
            .child(self.system_audio_control(cx))
            .child(
                canvas(
                    move |bounds, _, _| {
                        dimensions
                            .set([f32::from(bounds.size.width), f32::from(bounds.size.height)])
                    },
                    move |bounds, _, window, cx| {
                        paint_spectrum(bounds, &bands, &options, accent, window, cx)
                    },
                )
                .size_full()
                .flex_1(),
            )
            .into_any_element()
    }
}

impl Feishin {
    pub fn choose_milkdrop_preset(&mut self, name: String, cx: &mut Context<Self>) {
        if visualizer::milkdrop_data(&self.state.settings.visualizer.butterchurn, &name).is_none() {
            return;
        }
        self.state.settings.visualizer.butterchurn["currentPreset"] = json!(name);
        self.milkdrop_cycle_elapsed = 0.;
        self.milkdrop_error = None;
        self.save();
        cx.notify();
    }
    pub fn advance_milkdrop(&mut self, reverse: bool, cx: &mut Context<Self>) {
        if let Some(name) = visualizer::milkdrop_next(
            &self.state.settings.visualizer.butterchurn,
            reverse,
            rand::random(),
        ) {
            self.choose_milkdrop_preset(name, cx);
        }
    }
    fn toggle_milkdrop_list(&mut self, key: &str, name: String, cx: &mut Context<Self>) {
        let options = &mut self.state.settings.visualizer.butterchurn;
        let mut names = options[key].as_array().cloned().unwrap_or_default();
        if names.iter().any(|value| value.as_str() == Some(&name)) {
            names.retain(|value| value.as_str() != Some(&name));
        } else {
            names.push(json!(name));
        }
        options[key] = json!(names);
        self.save();
        cx.notify();
    }
    fn import_milkdrop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Import MilkDrop .milk presets".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await {
                let task = cx.background_executor().spawn(async move {
                    use std::io::Read;
                    anyhow::ensure!(paths.len() <= 100, "Import at most 100 presets at once");
                    let mut presets = vec![];
                    for path in paths {
                        anyhow::ensure!(
                            path.extension()
                                .is_some_and(|extension| extension == "milk"),
                            "Choose a .milk preset"
                        );
                        let name = path
                            .file_stem()
                            .context("Preset has no name")?
                            .to_string_lossy()
                            .into_owned();
                        let mut bytes = vec![];
                        std::fs::File::open(&path)?
                            .take(1_000_001)
                            .read_to_end(&mut bytes)?;
                        anyhow::ensure!(bytes.len() <= 1_000_000, "Preset is too large");
                        presets.push((name, String::from_utf8(bytes)?));
                    }
                    Ok::<_, anyhow::Error>(presets)
                });
                let result = task.await;
                this.update(cx, |this, cx| {
                    let result = result.and_then(|presets| {
                        let mut config = this.state.settings.visualizer.clone();
                        let mut custom = config.butterchurn["customPresets"]
                            .as_object()
                            .cloned()
                            .unwrap_or_default();
                        for (name, data) in presets {
                            custom.insert(name, json!(data));
                        }
                        config.butterchurn["customPresets"] = json!(custom);
                        visualizer::validate(&config)?;
                        this.state.settings.visualizer = config;
                        this.save();
                        Ok(())
                    });
                    this.message = match result {
                        Ok(()) => "MilkDrop presets imported".into(),
                        Err(error) => error.to_string(),
                    };
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
    fn milkdrop_view(
        &mut self,
        compact: bool,
        interactive: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let elapsed = self.milkdrop_preset_at.elapsed().as_secs_f32().min(1.);
        self.milkdrop_preset_at = std::time::Instant::now();
        if self.playing {
            self.milkdrop_cycle_elapsed += elapsed;
            if enabled(&self.state.settings.visualizer.butterchurn, "cyclePresets")
                && self.milkdrop_cycle_elapsed
                    >= number(
                        &self.state.settings.visualizer.butterchurn,
                        "cycleTime",
                        30.,
                    )
            {
                self.advance_milkdrop(false, cx);
            }
        }
        if self.milkdrop.is_none()
            && self.milkdrop_error.is_none()
            && !cfg!(test)
            && HasWindowHandle::window_handle(window).is_ok()
        {
            match crate::milkdrop::Renderer::new(&self.data_dir) {
                Ok(renderer) => {
                    self.milkdrop = Some(std::rc::Rc::new(std::cell::RefCell::new(renderer)))
                }
                Err(error) => {
                    log::warn!("MilkDrop initialization failed: {error}");
                    self.milkdrop_error = Some(error.to_string());
                }
            }
        }
        if (self.playing || self.system_audio.is_some()) && self.milkdrop_error.is_none() {
            window.request_animation_frame();
        }
        let options = self.state.settings.visualizer.butterchurn.clone();
        let current = options["currentPreset"]
            .as_str()
            .filter(|name| visualizer::milkdrop_data(&options, name).is_some())
            .unwrap_or(&visualizer::milkdrop_presets().names[0])
            .to_owned();
        let filter = self.milkdrop_search.read(cx).value().to_lowercase();
        let names = visualizer::milkdrop_names(&options)
            .into_iter()
            .filter(|name| name.to_lowercase().contains(&filter))
            .collect::<Vec<_>>();
        let renderer = if self.milkdrop_error.is_none() {
            self.milkdrop.clone()
        } else {
            None
        };
        let opacity = number(&options, "opacity", 1.);
        let capture = self.visualizer_capture();
        let playing = self.visualizer_has_audio();
        let entity = cx.weak_entity();
        let frame_options = options.clone();
        let surface = div().opacity(opacity).size_full().flex_1().child(
            canvas(
                move |_, _, _| (),
                move |bounds, _, window, cx| {
                    let Some(renderer) = &renderer else { return };
                    match renderer.borrow_mut().frame(
                        f32::from(bounds.size.width) * window.scale_factor(),
                        f32::from(bounds.size.height) * window.scale_factor(),
                        &frame_options,
                        &capture,
                        playing,
                    ) {
                        Ok((image, old)) => {
                            if let Some(old) = old {
                                let _ = window.drop_image(old);
                            }
                            let _ = window.paint_image(bounds, Corners::default(), image, 0, false);
                        }
                        Err(error) => {
                            log::warn!("MilkDrop rendering failed: {error}");
                            entity
                                .update(cx, |this, cx| {
                                    this.milkdrop_error = Some(error.to_string());
                                    cx.notify();
                                })
                                .ok();
                        }
                    }
                },
            )
            .size_full(),
        );
        if compact {
            return if interactive {
                self.visualizer_compact_frame(surface.into_any_element(), window, cx)
            } else {
                surface.into_any_element()
            };
        }
        div()
            .id("milkdrop-visualizer")
            .debug_selector(|| "milkdrop-visualizer".into())
            .size_full()
            .v_flex()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_2()
                    .child(
                        crate::styles::button("milkdrop-settings", cx)
                            .label(crate::appearance::text("Settings", cx))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.edit_visualizer(window, cx)),
                            ),
                    )
                    .child(
                        crate::styles::button("milkdrop-fullscreen", cx)
                            .label(self.tr("common.maximize"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_expanded_visualizer(window, cx)
                            })),
                    )
                    .child(
                        crate::styles::button("switch-spectrum", cx)
                            .label(crate::appearance::text("Spectrum analyzer", cx))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.settings.visualizer.kind = "audiomotionanalyzer".into();
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button("milkdrop-previous", cx)
                            .label(crate::appearance::text("Previous preset", cx))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.advance_milkdrop(true, cx)),
                            ),
                    )
                    .child(
                        crate::styles::button("milkdrop-next", cx)
                            .label(crate::appearance::text("Next preset", cx))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.advance_milkdrop(false, cx)),
                            ),
                    )
                    .child(crate::styles::input(&self.milkdrop_search, cx).w_56())
                    .child(
                        crate::styles::button("milkdrop-preset", cx)
                            .label(current.clone())
                            .dropdown_menu({
                                let entity = cx.entity();
                                move |mut menu, _, _| {
                                    menu = menu.scrollable(true).max_h(px(500.));
                                    for name in &names {
                                        let name = name.clone();
                                        let entity = entity.clone();
                                        menu =
                                            menu.item(PopupMenuItem::new(name.clone()).on_click(
                                                move |_, _, cx| {
                                                    entity.update(cx, |this, cx| {
                                                        this.choose_milkdrop_preset(
                                                            name.clone(),
                                                            cx,
                                                        )
                                                    });
                                                },
                                            ));
                                    }
                                    menu
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_2()
                    .children(
                        [
                            ("cyclePresets", "Cycle"),
                            ("randomizeNextPreset", "Random"),
                            ("includeAllPresets", "Use all presets"),
                        ]
                        .into_iter()
                        .map(|(key, label)| {
                            crate::styles::button(key, cx)
                                .label(format!("{label}: {}", enabled(&options, key)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let options = &mut this.state.settings.visualizer.butterchurn;
                                    options[key] = json!(!enabled(options, key));
                                    this.milkdrop_cycle_elapsed = 0.;
                                    this.save();
                                    cx.notify();
                                }))
                        }),
                    )
                    .child(
                        crate::styles::button("milkdrop-favorite", cx)
                            .label(
                                if options["selectedPresets"]
                                    .as_array()
                                    .is_some_and(|p| p.iter().any(|p| p.as_str() == Some(&current)))
                                {
                                    "Unselect preset"
                                } else {
                                    "Select preset"
                                },
                            )
                            .on_click({
                                let current = current.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.toggle_milkdrop_list(
                                        "selectedPresets",
                                        current.clone(),
                                        cx,
                                    )
                                })
                            }),
                    )
                    .child(
                        crate::styles::button("milkdrop-ignore", cx)
                            .label(
                                if options["ignoredPresets"]
                                    .as_array()
                                    .is_some_and(|p| p.iter().any(|p| p.as_str() == Some(&current)))
                                {
                                    "Include in cycle"
                                } else {
                                    "Exclude from cycle"
                                },
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_milkdrop_list("ignoredPresets", current.clone(), cx)
                            })),
                    )
                    .child(
                        crate::styles::button("milkdrop-import", cx)
                            .label(crate::appearance::text("Import presets", cx))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.import_milkdrop(window, cx)),
                            ),
                    ),
            )
            .child(self.system_audio_control(cx))
            .when_some(self.milkdrop_error.clone(), |div, error| {
                div.child(error).child(
                    crate::styles::button("milkdrop-retry", cx)
                        .label(crate::appearance::text("Retry", cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.milkdrop_error = None;
                            cx.notify();
                        })),
                )
            })
            .child(surface)
            .into_any_element()
    }
}

fn gradient_stop(
    value: &Value,
    window: &mut Window,
    cx: &mut Context<Feishin>,
) -> (GradientStop, Vec<Subscription>) {
    let text = value
        .as_str()
        .or_else(|| value["color"].as_str())
        .unwrap_or("#ffffff");
    let rgba = feishin_core::appearance::color(text).unwrap_or([1.; 4]);
    let color: Hsla = Rgba {
        r: rgba[0],
        g: rgba[1],
        b: rgba[2],
        a: rgba[3],
    }
    .into();
    let color = cx.new(|cx| ColorPickerState::new(window, cx).default_value(color));
    let position =
        cx.new(|cx| crate::appearance::input_state(window, cx).aria_label("Gradient position"));
    let level =
        cx.new(|cx| crate::appearance::input_state(window, cx).aria_label("Gradient level"));
    let mut subscriptions = Vec::new();
    for (key, input) in [("pos", &position), ("level", &level)] {
        input.update(cx, |input, cx| {
            input.set_value(number(value, key, 0.).to_string(), window, cx)
        });
        subscriptions.push(cx.subscribe_in(
            input,
            window,
            |_, input, event: &NumberInputEvent, window, cx| {
                let NumberInputEvent::Step(step) = event;
                let value = input.read(cx).value().parse::<f32>().unwrap_or_default();
                let value = (value
                    + if *step == StepAction::Increment {
                        0.1
                    } else {
                        -0.1
                    })
                .clamp(0., 1.);
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
            },
        ));
    }
    (
        GradientStop {
            color,
            position,
            level,
            position_enabled: enabled(value, "positionEnabled"),
            level_enabled: enabled(value, "levelEnabled"),
        },
        subscriptions,
    )
}

fn color(options: &Value, channel: usize, index: f32, level: f64, accent: Hsla) -> Hsla {
    let mut color = spectrum_color(options, channel, index, level, accent).to_rgb();
    let brightness = number(options, "_reflexBright", 1.);
    color.r = (color.r * brightness).clamp(0., 1.);
    color.g = (color.g * brightness).clamp(0., 1.);
    color.b = (color.b * brightness).clamp(0., 1.);
    color.into()
}
fn spectrum_color(options: &Value, channel: usize, index: f32, level: f64, accent: Hsla) -> Hsla {
    let gradient = options[if options["channelLayout"].as_str().unwrap_or("single") == "single" {
        "gradient"
    } else if channel == 0 {
        "gradientLeft"
    } else {
        "gradientRight"
    }]
    .as_str()
    .or_else(|| options["gradient"].as_str())
    .unwrap_or("prism");
    let custom = options["customGradients"].as_array().and_then(|gradients| {
        gradients
            .iter()
            .find(|custom| custom["name"].as_str() == Some(gradient))
    });
    let definition = custom.unwrap_or(&visualizer::spectrum_gradients()[gradient]);
    let Some(stops) = definition["colorStops"]
        .as_array()
        .filter(|stops| !stops.is_empty())
    else {
        return accent;
    };
    let mut colors: Vec<(f32, f64, Hsla)> = stops
        .iter()
        .enumerate()
        .filter_map(|(index, stop)| {
            let text = stop.as_str().or_else(|| stop["color"].as_str())?;
            let rgba = feishin_core::appearance::color(text).ok()?;
            let position = if custom.is_none() || enabled(stop, "positionEnabled") {
                number(
                    stop,
                    "pos",
                    index as f32 / stops.len().saturating_sub(1).max(1) as f32,
                )
            } else {
                index as f32 / stops.len().saturating_sub(1).max(1) as f32
            };
            let threshold = if custom.is_none() || enabled(stop, "levelEnabled") {
                stop["level"]
                    .as_f64()
                    .unwrap_or(1. - index as f64 / stops.len() as f64)
            } else {
                1. - index as f64 / stops.len() as f64
            };
            Some((
                position,
                threshold,
                Rgba {
                    r: rgba[0],
                    g: rgba[1],
                    b: rgba[2],
                    a: rgba[3],
                }
                .into(),
            ))
        })
        .collect();
    if colors.is_empty() {
        return accent;
    }
    colors.sort_by(|a, b| b.1.total_cmp(&a.1));
    colors[0].1 = 1.;
    let flags = visualizer::ModeFlags::new(options);
    let mode = options["colorMode"].as_str().unwrap_or("gradient");
    if !flags.graph && mode == "bar-index" {
        let bar = (index * (number(options, "_bandCount", 1.) - 1.).max(0.)).round() as usize;
        return colors[bar % colors.len()].2;
    }
    if !flags.graph
        && (mode == "bar-level"
            || (flags.leds && enabled(options, "trueLeds") && mode == "gradient"))
    {
        let threshold = |value: f64| {
            if flags.leds && number(options, "_ledCount", 0.) > 0. {
                (value * f64::from(number(options, "_ledCount", 0.))).floor()
            } else {
                value
            }
        };
        return colors
            .iter()
            .rfind(|color| threshold(level.clamp(0., 1.)) <= threshold(color.1))
            .unwrap_or(&colors[0])
            .2;
    }
    let radial = enabled(options, "radial");
    let vertical = options["channelLayout"].as_str() == Some("dual-vertical");
    let horizontal = definition["dir"].as_str() == Some("h");
    let dual = vertical && !enabled(options, "splitGradient") && (!horizontal || radial);
    let reflection = number(options, "reflexRatio", 0.5);
    let canvas_size = [0, 1].map(|index| {
        options["_canvasSize"][index]
            .as_f64()
            .map_or([1280., 540.][index], |value| value as f32)
    });
    let geometry = visualizer::spectrum_geometry(canvas_size, options, 1.);
    let position = if radial {
        let (inner, reach) = visualizer::radial_dimensions(canvas_size, options, 0);
        let outer = inner + reach;
        let radius = inner + reach * level as f32 * if vertical && channel == 1 { -1. } else { 1. };
        (radius - outer) / (inner - reach * f32::from(vertical) - outer)
    } else if horizontal {
        index
    } else {
        let gradient_height = if flags.lumi {
            canvas_size[1]
        } else {
            (canvas_size[1] * (1. - reflection * f32::from(!vertical))).floor()
        };
        let top = if vertical {
            (geometry.channel_height + geometry.channel_gap) * channel as f32
        } else {
            0.
        };
        (top + geometry.height * (1. - level as f32)) / gradient_height.max(1.)
    };
    let mut stops = Vec::with_capacity(colors.len() * if dual { 2 } else { 1 } + 2);
    for area in 0..=usize::from(dual) {
        for (index, &(pos, _, color)) in colors.iter().enumerate() {
            let mut offset = pos / if dual { 2. } else { 1. };
            let mut color = color;
            if vertical && !flags.lumi && !radial && !horizontal {
                offset *= 1. - reflection;
                if !dual && offset > 0.5 * (1. - reflection) {
                    offset += 0.5 * reflection;
                }
            }
            if area == 1 {
                if radial || flags.lumi {
                    let reversed = colors[colors.len() - 1 - index];
                    color = reversed.2;
                    offset = 1. - reversed.0 / 2.;
                } else {
                    if index == 0 && offset > 0. {
                        stops.push((0.5, color));
                    }
                    offset += 0.5;
                }
            }
            stops.push((offset, color));
            if vertical && index == colors.len() - 1 && offset < 0.5 {
                stops.push((0.5, color));
            }
        }
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    interpolate_color(&stops, position, false, accent)
}

fn peak_color(
    options: &Value,
    channel: usize,
    index: f32,
    band: &visualizer::Band,
    accent: Hsla,
) -> Hsla {
    let mut tint = color(
        options,
        channel,
        index,
        f64::from(band.peaks[channel]),
        accent,
    );
    tint.a *= number(options, "opacity", 1.);
    tint.a *= if enabled(options, "fadePeaks") {
        band.peak_alpha[channel]
    } else if visualizer::ModeFlags::new(options).alpha {
        band.peaks[channel]
    } else {
        1.
    };
    tint
}
fn interpolate_color(stops: &[(f32, Hsla)], value: f32, stepped: bool, fallback: Hsla) -> Hsla {
    let Some(&(first, first_color)) = stops.first() else {
        return fallback;
    };
    if value <= first {
        return first_color;
    }
    if let Some(&(last, color)) = stops.last()
        && value >= last
    {
        return color;
    }
    for pair in stops.windows(2) {
        let [(low, left), (high, right)] = pair else {
            unreachable!()
        };
        if value <= *high {
            if stepped {
                return *left;
            }
            let mix = ((value - low) / (high - low).max(0.0001)).clamp(0., 1.);
            let left = left.to_rgb();
            let right = right.to_rgb();
            return Rgba {
                r: left.r + (right.r - left.r) * mix,
                g: left.g + (right.g - left.g) * mix,
                b: left.b + (right.b - left.b) * mix,
                a: left.a + (right.a - left.a) * mix,
            }
            .into();
        }
    }
    stops.last().unwrap().1
}
fn horizontal_gradient(options: &Value, channel: usize) -> bool {
    let key = if options["channelLayout"].as_str().unwrap_or("single") == "single" {
        "gradient"
    } else if channel == 0 {
        "gradientLeft"
    } else {
        "gradientRight"
    };
    let name = options[key]
        .as_str()
        .or_else(|| options["gradient"].as_str())
        .unwrap_or("prism");
    name == "rainbow"
        || (number(options, "mode", 10.) != 10.
            && options["colorMode"].as_str() == Some("bar-index"))
        || options["customGradients"]
            .as_array()
            .is_some_and(|gradients| {
                gradients.iter().any(|gradient| {
                    gradient["name"].as_str() == Some(name) && gradient["dir"].as_str() == Some("h")
                })
            })
}
fn radial_angle(
    position: f32,
    layout: &str,
    channel: usize,
    mirror: f32,
    copy: usize,
    spin: f32,
    canvas: [f32; 2],
) -> f32 {
    let x = position * canvas[1];
    let (offset, direction) = if layout == "dual-horizontal" {
        let inverted = (channel == 0 && mirror < 0.) || (channel == 1 && mirror > 0.);
        let offset = if channel == 0 {
            if mirror < 0. {
                canvas[0]
            } else {
                (canvas[0] / 2.).floor()
            }
        } else if mirror > 0. {
            (canvas[0] / 2.).floor()
        } else {
            0.
        };
        (offset, if inverted { -1. } else { 1. })
    } else if mirror != 0. {
        (0., if copy == 0 { 1. } else { -1. })
    } else {
        (0., 1.)
    };
    let turns = (x + offset) / canvas[0] * direction;
    turns * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2 + spin
}
fn radial_level(radius: f32, reach: f32, level: f32, inverted: bool) -> f32 {
    (radius + reach * level.clamp(0., 1.) * if inverted { -1. } else { 1. }).max(0.)
}
fn radial_bar_path(
    center: Point<Pixels>,
    angles: [f32; 2],
    radii: [f32; 2],
    round: bool,
    stroke: Option<f32>,
) -> PathBuilder {
    let mut path = stroke.map_or_else(PathBuilder::fill, |width| PathBuilder::stroke(px(width)));
    push_radial_bar(&mut path, center, angles, radii, round, stroke.is_none());
    path
}
fn push_radial_bar(
    path: &mut PathBuilder,
    center: Point<Pixels>,
    angles: [f32; 2],
    radii: [f32; 2],
    round: bool,
    closed: bool,
) {
    let position = |angle, radius| radial_point(center, angle, radius);
    path.move_to(position(angles[0], radii[0]));
    path.line_to(position(angles[0], radii[1]));
    if round {
        path.arc_to(
            point(px(radii[1].abs()), px(radii[1].abs())),
            px(0.),
            false,
            angles[1] > angles[0],
            position(angles[1], radii[1]),
        );
    } else {
        path.line_to(position(angles[1], radii[1]));
    }
    path.line_to(position(angles[1], radii[0]));
    if closed {
        if round {
            path.arc_to(
                point(px(radii[0].abs()), px(radii[0].abs())),
                px(0.),
                false,
                angles[0] > angles[1],
                position(angles[0], radii[0]),
            );
        }
        path.close();
    }
}
fn radial_point(center: Point<Pixels>, angle: f32, radius: f32) -> Point<Pixels> {
    center + point(px(angle.cos() * radius), px(angle.sin() * radius))
}
#[derive(Clone, Copy)]
struct SpectrumProjection {
    source_bottom: f32,
    scale: f32,
    reflected: bool,
    clip: Bounds<Pixels>,
}
impl SpectrumProjection {
    fn transform(self) -> Transform {
        if self.reflected {
            Transform::new(
                1.,
                0.,
                0.,
                -self.scale,
                0.,
                self.source_bottom * (1. + self.scale),
            )
        } else {
            Transform::identity()
        }
    }
    fn bounds(self, mut bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        if self.reflected {
            bounds.origin.y =
                px(self.source_bottom
                    + (self.source_bottom - f32::from(bounds.bottom())) * self.scale);
            bounds.size.height *= self.scale;
        }
        bounds
    }
    fn paint_quad(self, mut quad: PaintQuad, window: &mut Window) {
        quad.bounds = self.bounds(quad.bounds);
        if self.reflected {
            quad.corner_radii = Corners {
                top_left: quad.corner_radii.bottom_left,
                top_right: quad.corner_radii.bottom_right,
                bottom_left: quad.corner_radii.top_left,
                bottom_right: quad.corner_radii.top_right,
            };
            std::mem::swap(&mut quad.border_widths.top, &mut quad.border_widths.bottom);
            quad.border_widths.top *= self.scale;
            quad.border_widths.bottom *= self.scale;
        }
        window.with_content_mask(Some(ContentMask { bounds: self.clip }), |window| {
            window.paint_quad(quad)
        });
    }
}
fn paint_colored_path(
    path: gpui::Path<Pixels>,
    bounds: Bounds<Pixels>,
    options: &Value,
    channel: (usize, bool),
    tint: (Hsla, f32),
    projection: SpectrumProjection,
    window: &mut Window,
) {
    let horizontal = horizontal_gradient(options, channel.0);
    // GPUI paths accept two color stops. Bounded strips preserve the full palette,
    // including custom intermediate stops, without retaining a raster per frame.
    let strips = 64;
    for strip in 0..strips {
        let low = strip as f32 / strips as f32;
        let high = (strip + 1) as f32 / strips as f32;
        let level = (low + high) * 0.5;
        let mut color = color(
            options,
            channel.0,
            if channel.1 { 1. - level } else { level },
            f64::from(level),
            tint.0,
        );
        color.a *= tint.1;
        let mask = if horizontal {
            Bounds::new(
                bounds.origin + point(bounds.size.width * low, px(0.)),
                size(bounds.size.width * (high - low), bounds.size.height),
            )
        } else {
            Bounds::new(
                bounds.origin + point(px(0.), bounds.size.height * (1. - high)),
                size(bounds.size.width, bounds.size.height * (high - low)),
            )
        };
        let mask = projection.bounds(mask).intersect(&projection.clip);
        window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
            window.paint_path(path.clone(), color)
        });
    }
}
fn paint_gradient_bar(
    quad: PaintQuad,
    options: &Value,
    bar: (usize, f32),
    height: f32,
    tint: Hsla,
    projection: SpectrumProjection,
    window: &mut Window,
) {
    let (channel, index) = bar;
    if horizontal_gradient(options, channel)
        || options["colorMode"].as_str().unwrap_or("gradient") != "gradient"
    {
        projection.paint_quad(quad, window);
        return;
    }
    let bar_height = f32::from(quad.bounds.size.height);
    if bar_height <= 0. {
        return;
    }
    let steps = (bar_height / 4.).ceil().clamp(1., 64.) as usize;
    let bottom_level =
        (projection.source_bottom - f32::from(quad.bounds.origin.y) - bar_height) / height;
    let outline = visualizer::ModeFlags::new(options).outline;
    for step in 0..steps {
        let low = step as f32 / steps as f32;
        let high = (step + 1) as f32 / steps as f32;
        let mut lower = color(
            options,
            channel,
            index,
            f64::from(bottom_level + low * bar_height / height),
            tint,
        );
        let mut upper = color(
            options,
            channel,
            index,
            f64::from(bottom_level + high * bar_height / height),
            tint,
        );
        lower.a *= tint.a;
        upper.a *= tint.a;
        let mut border = color(
            options,
            channel,
            index,
            f64::from(bottom_level + (low + high) * 0.5 * bar_height / height),
            tint,
        );
        border.a *= number(options, "opacity", 1.);
        if outline {
            lower.a *= number(options, "fillAlpha", 0.);
            upper.a *= number(options, "fillAlpha", 0.);
        }
        let mask = Bounds::new(
            quad.bounds.origin + point(px(0.), quad.bounds.size.height * (1. - high)),
            size(
                quad.bounds.size.width,
                quad.bounds.size.height * (high - low),
            ),
        );
        let colored = quad
            .clone()
            .border_color(border)
            .background(linear_gradient(
                if projection.reflected { 180. } else { 0. },
                linear_color_stop(lower, low),
                linear_color_stop(upper, high),
            ));
        let mask = projection.bounds(mask);
        window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
            projection.paint_quad(colored, window)
        });
    }
}
fn paint_radial_gradient(
    bounds: Bounds<Pixels>,
    bands: &[visualizer::Band],
    options: &Value,
    channel_copy: (usize, usize),
    geometry: &visualizer::SpectrumGeometry,
    accent: Hsla,
    window: &mut Window,
) {
    let (channel, copy) = channel_copy;
    let pixel_ratio = number(options, "_pixelRatio", 1.).max(0.1);
    let canvas_size = [bounds.size.width, bounds.size.height]
        .map(|dimension| (f32::from(dimension) * pixel_ratio).floor());
    let center = bounds.origin
        + point(
            px((canvas_size[0] / 2.).floor() / pixel_ratio),
            px((canvas_size[1] / 2.).floor() / pixel_ratio),
        );
    let layout = options["channelLayout"].as_str().unwrap_or("single");
    let mirror = number(options, "mirror", 0.);
    let flags = visualizer::ModeFlags::new(options);
    let (radius, reach) = visualizer::radial_dimensions(canvas_size, options, channel);
    let position = |angle: f32, level: f32| {
        radial_point(
            center,
            angle,
            (radius + (reach * level).trunc()) / pixel_ratio,
        )
    };
    let mut points: Vec<_> = geometry
        .bar_bounds(bands, options)
        .into_iter()
        .zip(bands)
        .map(|((x, width), band)| {
            (
                radial_angle(
                    ((x - geometry.initial_x) / geometry.width).max(0.),
                    layout,
                    channel,
                    mirror,
                    copy,
                    number(options, "_spin", 0.),
                    [canvas_size[0], geometry.width],
                ),
                band.values[channel] as f32,
                band.peaks[channel],
                radial_angle(
                    (x - geometry.initial_x + width) / geometry.width,
                    layout,
                    channel,
                    mirror,
                    copy,
                    number(options, "_spin", 0.),
                    [canvas_size[0], geometry.width],
                ),
            )
        })
        .collect();
    if flags.graph
        && mirror == 0.
        && layout != "dual-horizontal"
        && let Some(&first) = points.first()
    {
        points.push((first.0 + std::f32::consts::TAU, first.1, first.2, first.3));
    }
    // Match the rectangular graph's bounded gradient strips along the radius.
    // Each strip shares one path/color across the FFT bands.
    let fill_alpha = if flags.graph || flags.outline {
        number(options, "fillAlpha", 0.)
    } else {
        1.
    };
    if fill_alpha > 0. {
        for strip in 0..64 {
            let low = strip as f32 / 64.;
            let high = (strip + 1) as f32 / 64.;
            let mut path = PathBuilder::fill();
            if flags.graph {
                for pair in points.windows(2) {
                    let [(left, a, _, _), (right, b, _, _)] = pair else {
                        unreachable!()
                    };
                    if a.max(*b) <= low {
                        continue;
                    }
                    path.move_to(position(*left, a.min(low)));
                    path.line_to(position(*left, a.min(high)));
                    path.line_to(position(*right, b.min(high)));
                    path.line_to(position(*right, b.min(low)));
                    path.close();
                }
            } else {
                for &(angle, value, _, end_angle) in &points {
                    let radii = [low, high]
                        .map(|level| (radius + (reach * value.min(level)).trunc()) / pixel_ratio);
                    if radii[0] != radii[1] {
                        push_radial_bar(
                            &mut path,
                            center,
                            [angle, end_angle],
                            radii,
                            flags.round,
                            true,
                        );
                    }
                }
            }
            if let Ok(path) = path.build() {
                let mut tint = color(options, channel, 0., f64::from((low + high) / 2.), accent);
                tint.a *= fill_alpha * number(options, "opacity", 1.);
                window.paint_path(path, tint);
            }
        }
    }
    if !flags.graph {
        return;
    }
    let line_width = number(options, "lineWidth", 1.9) / pixel_ratio;
    let peak_line = enabled(options, "showPeaks") && visualizer::ModeFlags::new(options).peak_line;
    for pair in points.windows(2) {
        let [(left, a, peak_a, _), (right, b, peak_b, _)] = pair else {
            unreachable!()
        };
        for (width, a, b) in [
            (line_width, *a, *b),
            (f32::from(peak_line) / pixel_ratio, *peak_a, *peak_b),
        ] {
            if width <= 0. {
                continue;
            }
            let mut path = PathBuilder::stroke(px(width));
            path.move_to(position(*left, a));
            path.line_to(position(*right, b));
            if let Ok(path) = path.build() {
                let mut tint = color(options, channel, 0., f64::from((a + b) / 2.), accent);
                tint.a *= number(options, "opacity", 1.);
                window.paint_path(path, tint);
            }
        }
    }
}
fn paint_spectrum(
    bounds: Bounds<Pixels>,
    bands: &[visualizer::Band],
    options: &Value,
    accent: Hsla,
    window: &mut Window,
    cx: &mut App,
) {
    if bands.is_empty() {
        return;
    }
    let radial = enabled(options, "radial");
    let flags = visualizer::ModeFlags::new(options);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let pixel_ratio = number(options, "_pixelRatio", 1.).max(0.1);
    let canvas_size = [width, height].map(|dimension| (dimension * pixel_ratio).floor());
    let geometry = visualizer::spectrum_geometry(canvas_size, options, pixel_ratio);
    let bar_bounds = geometry.bar_bounds(bands, options);
    let mut measured_options = options.clone();
    measured_options["_ledCount"] = json!(geometry.leds.map_or(0, |leds| leds.0));
    measured_options["_canvasSize"] = json!(canvas_size);
    let options = &measured_options;
    let layout = options["channelLayout"].as_str().unwrap_or("single");
    let channels = if layout == "single" { 1 } else { 2 };
    for channel in 0..channels {
        let channel_width = if layout == "dual-horizontal" {
            geometry.width / pixel_ratio
        } else {
            width
        };
        let ch = if layout == "dual-vertical" && !radial {
            geometry.channel_height / pixel_ratio
        } else {
            height
        };
        let ox = if layout == "dual-horizontal" {
            channel_width * channel as f32
        } else {
            0.
        };
        let oy = if layout == "dual-vertical" && !radial {
            (geometry.channel_height + geometry.channel_gap) / pixel_ratio * channel as f32
        } else {
            0.
        };
        let reflection = if radial || flags.lumi {
            0.
        } else {
            number(options, "reflexRatio", 0.5).clamp(0., 0.99)
        };
        let graph_height = geometry.height / pixel_ratio;
        let analyzer_bottom = oy + graph_height
            - if geometry.no_led_gap {
                0.
            } else {
                geometry.leds.map_or(0., |leds| leds.2) / pixel_ratio
            };
        let mirror = number(options, "mirror", 0.);
        let copies = if mirror == 0. || layout == "dual-horizontal" {
            1
        } else {
            2
        };
        let cw = if copies == 2 {
            geometry.width / pixel_ratio
        } else {
            channel_width
        };
        for reflected in [false, true]
            .into_iter()
            .take(if reflection > 0. { 2 } else { 1 })
        {
            let mut reflection_options = options.clone();
            if reflected {
                reflection_options["opacity"] =
                    json!(number(options, "opacity", 1.) * number(options, "reflexAlpha", 0.15));
                reflection_options["_reflexBright"] = json!(number(options, "reflexBright", 1.));
            }
            let options = &reflection_options;
            let source_bottom = f32::from(bounds.origin.y) + oy + graph_height;
            let projection = SpectrumProjection {
                source_bottom,
                reflected,
                scale: if enabled(options, "reflexFit") || layout == "dual-vertical" {
                    (ch - graph_height) / graph_height.max(1.)
                } else {
                    1.
                },
                clip: Bounds::new(
                    bounds.origin
                        + point(px(0.), px(oy + if reflected { graph_height } else { 0. })),
                    size(
                        bounds.size.width,
                        px(if reflected {
                            ch - graph_height
                        } else {
                            graph_height
                        }),
                    ),
                ),
            };
            for copy in 0..copies {
                let ox = ox + copy as f32 * cw;

                let graph = number(options, "mode", 10.) == 10.;
                let line_width = number(options, "lineWidth", 1.9) / pixel_ratio;
                let mut line = PathBuilder::stroke(px(line_width.max(0.01)));
                let mut area = PathBuilder::fill();
                let mut peaks = PathBuilder::stroke(px(1. / pixel_ratio));
                if reflected {
                    line.transform(projection.transform());
                    area.transform(projection.transform());
                    peaks.transform(projection.transform());
                }
                let reversed = if layout == "dual-horizontal" {
                    (channel == 0 && mirror < 0.) || (channel == 1 && mirror > 0.)
                } else if mirror < 0. {
                    copy == 0
                } else {
                    copy == 1
                };
                let mut last_origin = bounds.origin;
                if !reflected && enabled(options, "showScaleY") && !radial {
                    let scale_width =
                        (20. * pixel_ratio).max((canvas_size[0].min(canvas_size[1]) / 32.).floor());
                    let font_size = px((scale_width / 2.).floor() / pixel_ratio);
                    for row in
                        visualizer::amplitude_scale(options, canvas_size, scale_width, channel)
                    {
                        let x = |x: f32| {
                            ox + if reversed {
                                cw - (x - geometry.initial_x) / pixel_ratio
                            } else {
                                (x - geometry.initial_x) / pixel_ratio
                            }
                        };
                        let mut path = PathBuilder::stroke(px(1. / pixel_ratio));
                        let period = if row.major { 6. } else { 10. };
                        let mut dash = row.start - f32::from(!row.major);
                        while dash < row.end {
                            path.move_to(
                                bounds.origin
                                    + point(px(x(dash.max(row.start))), px(row.y / pixel_ratio)),
                            );
                            path.line_to(
                                bounds.origin
                                    + point(
                                        px(x((dash + 2.).min(row.end))),
                                        px(row.y / pixel_ratio),
                                    ),
                            );
                            dash += period;
                        }
                        let tint: Hsla = rgb(if row.major { 0x888888 } else { 0x555555 }).into();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, tint);
                        }
                        for position in row.labels {
                            let text: SharedString = row.value.to_string().into();
                            let run = TextRun {
                                letter_spacing_fraction: 0.,
                                font_size: None,
                                line_height: None,
                                len: text.len(),
                                font: font(cx.theme().font_family.clone()),
                                color: tint,
                                background_color: None,
                                underline: None,
                                overline: None,
                                strikethrough: None,
                                letter_spacing: gpui::px(0.),
                                text_spacing_trim: gpui::TextSpacingTrim::Normal,
                                text_direction: Default::default(),
                            };
                            let line =
                                window
                                    .text_system()
                                    .shape_line(text, font_size, &[run], None);
                            let origin = bounds.origin
                                + point(
                                    px(x(position)) - if reversed { px(0.) } else { line.width },
                                    px(row.label_y / pixel_ratio) - line.ascent,
                                );
                            let _ = line.paint(origin, line.ascent + line.descent, window, cx);
                        }
                    }
                }
                let batched_radial = radial
                    && (graph
                        || !flags.alpha
                            && options["colorMode"].as_str().unwrap_or("gradient") == "gradient");
                if batched_radial {
                    paint_radial_gradient(
                        bounds,
                        bands,
                        options,
                        (channel, copy),
                        &geometry,
                        accent,
                        window,
                    );
                }
                for (i, band) in bands.iter().enumerate() {
                    let (bar_x, bar_width) = bar_bounds[i];
                    let index = if options["colorMode"].as_str() == Some("bar-index") && !graph {
                        i as f32 / (bands.len() - 1).max(1) as f32
                    } else {
                        (bar_x - geometry.initial_x + bar_width / 2.) / geometry.width
                    };
                    let bar_x = (bar_x - geometry.initial_x) / pixel_ratio;
                    let step = bar_width / pixel_ratio;
                    let x = if reversed { cw - bar_x - step } else { bar_x };
                    let value = band.values[channel] as f32;
                    let mut tint = color(options, channel, index, band.values[channel], accent);
                    tint.a *= number(options, "opacity", 1.);
                    if flags.alpha {
                        tint.a *= value;
                    }
                    if radial {
                        let angle = radial_angle(
                            bar_x / (geometry.width / pixel_ratio),
                            layout,
                            channel,
                            mirror,
                            copy,
                            number(options, "_spin", 0.),
                            [canvas_size[0], geometry.width],
                        );
                        let (radius, reach) =
                            visualizer::radial_dimensions(canvas_size, options, channel);
                        let radius = radius / pixel_ratio;
                        let reach = reach / pixel_ratio;
                        let end_radius =
                            radius + (reach * value * pixel_ratio).trunc() / pixel_ratio;
                        let center = bounds.origin
                            + point(
                                px((canvas_size[0] / 2.).floor() / pixel_ratio),
                                px((canvas_size[1] / 2.).floor() / pixel_ratio),
                            );
                        if !graph && end_radius != radius {
                            let angles = [
                                angle,
                                radial_angle(
                                    (bar_x + step) / (geometry.width / pixel_ratio),
                                    layout,
                                    channel,
                                    mirror,
                                    copy,
                                    number(options, "_spin", 0.),
                                    [canvas_size[0], geometry.width],
                                ),
                            ];
                            let gradient =
                                options["colorMode"].as_str().unwrap_or("gradient") == "gradient";
                            let strips = if batched_radial {
                                0
                            } else if gradient {
                                16
                            } else {
                                1
                            };
                            for strip in 0..strips {
                                let low = strip as f32 / strips as f32;
                                let high = (strip + 1) as f32 / strips as f32;
                                let mut fill_tint = if gradient {
                                    color(
                                        options,
                                        channel,
                                        index,
                                        f64::from(value * (low + high) * 0.5),
                                        accent,
                                    )
                                } else {
                                    tint
                                };
                                if gradient {
                                    fill_tint.a *= number(options, "opacity", 1.)
                                        * if flags.alpha { value } else { 1. };
                                }
                                if flags.outline {
                                    fill_tint.a *= number(options, "fillAlpha", 1.);
                                }
                                let radii = [
                                    radius + (end_radius - radius) * low,
                                    radius + (end_radius - radius) * high,
                                ];
                                if let Ok(path) =
                                    radial_bar_path(center, angles, radii, flags.round, None)
                                        .build()
                                {
                                    window.paint_path(path, fill_tint);
                                }
                            }
                            if flags.outline && line_width > 0. {
                                let mut outline = tint;
                                outline.a = number(options, "opacity", 1.);
                                if let Ok(path) = radial_bar_path(
                                    center,
                                    angles,
                                    [radius, end_radius],
                                    flags.round,
                                    Some(line_width.min(step / 2.)),
                                )
                                .build()
                                {
                                    window.paint_path(path, outline);
                                }
                            }
                        }
                        if enabled(options, "showPeaks") && !flags.lumi {
                            let peak_radius =
                                radial_level(radius, reach, band.peaks[channel], false);
                            let peak_origin = center
                                + point(
                                    px(angle.cos() * peak_radius),
                                    px(angle.sin() * peak_radius),
                                );
                            if i == 0 {
                                peaks.move_to(peak_origin);
                            } else {
                                peaks.line_to(peak_origin);
                            }
                            let peak_tint = peak_color(options, channel, index, band, accent);
                            if !flags.peak_line && band.peaks[channel] > 0. && peak_tint.a > 0. {
                                let peak_width = if graph { 1. / pixel_ratio } else { step };
                                let angles = [
                                    angle,
                                    radial_angle(
                                        (bar_x + peak_width) / (geometry.width / pixel_ratio),
                                        layout,
                                        channel,
                                        mirror,
                                        copy,
                                        number(options, "_spin", 0.),
                                        [canvas_size[0], geometry.width],
                                    ),
                                ];
                                let direction = if reach < 0. { 1. } else { -1. };
                                if let Ok(path) = radial_bar_path(
                                    center,
                                    angles,
                                    [
                                        peak_radius,
                                        (peak_radius + direction * 2. / pixel_ratio).max(0.),
                                    ],
                                    flags.round && !graph,
                                    None,
                                )
                                .build()
                                {
                                    window.paint_path(path, peak_tint);
                                }
                            }
                        }
                    } else {
                        let origin = bounds.origin
                            + point(
                                px(ox + x),
                                px(analyzer_bottom
                                    - (value * geometry.height).trunc() / pixel_ratio),
                            );
                        if graph {
                            if i == 0 {
                                line.move_to(origin);
                                area.move_to(
                                    bounds.origin + point(px(ox + x), px(oy + graph_height)),
                                );
                            } else {
                                line.line_to(origin);
                            }
                            area.line_to(origin);
                            last_origin = origin;
                        } else if flags.leds {
                            let (total, _, led_gap, led_height) = geometry.leds.unwrap();
                            let count = if flags.lumi {
                                total
                            } else {
                                (band.values[channel] * total as f64).floor() as usize
                            };
                            for led in 0..total {
                                if led >= count {
                                    continue;
                                }
                                let level = (led + 1) as f64 / total as f64;
                                let mut tint = color(
                                    options,
                                    channel,
                                    index,
                                    if options["colorMode"].as_str() == Some("bar-level") {
                                        band.values[channel]
                                    } else {
                                        level
                                    },
                                    accent,
                                );
                                tint.a *= number(options, "opacity", 1.)
                                    * if flags.lumi || flags.alpha { value } else { 1. };
                                let led_bounds = Bounds::new(
                                    bounds.origin
                                        + point(
                                            px(ox + x),
                                            px(analyzer_bottom
                                                - ((led + 1) as f32 * (led_height + led_gap)
                                                    - led_gap)
                                                    / pixel_ratio),
                                        ),
                                    size(px(step), px(led_height / pixel_ratio)),
                                );
                                let mut quad = fill(led_bounds, tint);
                                if options["colorMode"].as_str().unwrap_or("gradient") == "gradient"
                                    && !enabled(options, "trueLeds")
                                    && !horizontal_gradient(options, channel)
                                {
                                    let low = (source_bottom
                                        - f32::from(led_bounds.origin.y + led_bounds.size.height))
                                        / graph_height;
                                    let high =
                                        low + f32::from(led_bounds.size.height) / graph_height;
                                    let mut lower =
                                        color(options, channel, index, f64::from(low), accent);
                                    let mut upper =
                                        color(options, channel, index, f64::from(high), accent);
                                    lower.a *= tint.a;
                                    upper.a *= tint.a;
                                    quad = quad.background(linear_gradient(
                                        if reflected { 180. } else { 0. },
                                        linear_color_stop(lower, 0.),
                                        linear_color_stop(upper, 1.),
                                    ));
                                }
                                projection.paint_quad(quad, window);
                            }
                        } else {
                            let bar_bounds = Bounds::new(
                                if flags.lumi {
                                    bounds.origin + point(px(ox + x), px(oy))
                                } else {
                                    origin
                                },
                                size(
                                    px(step.max(0.5)),
                                    px(if flags.lumi {
                                        graph_height
                                    } else {
                                        (geometry.height * value).trunc() / pixel_ratio
                                    }),
                                ),
                            );
                            if flags.lumi {
                                tint.a *= value;
                            }
                            let mut quad = fill(bar_bounds, tint);
                            if flags.outline {
                                let mut background = tint;
                                background.a *= number(options, "fillAlpha", 0.);
                                quad = quad
                                    .background(background)
                                    .border_widths(px(line_width.min(step / 2.)))
                                    .border_color(tint);
                            }
                            if flags.round {
                                let radius = px(step.max(0.5) / 2.);
                                quad.bounds.size.height += radius;
                                quad = quad.corner_radii(Corners {
                                    top_left: radius,
                                    top_right: radius,
                                    bottom_left: px(0.),
                                    bottom_right: px(0.),
                                });
                            }
                            paint_gradient_bar(
                                quad,
                                options,
                                (channel, index),
                                graph_height,
                                tint,
                                projection,
                                window,
                            );
                        }
                        if enabled(options, "showPeaks") && !flags.lumi {
                            let peak_origin = bounds.origin
                                + point(
                                    px(ox + x),
                                    px(oy + graph_height * (1. - band.peaks[channel])),
                                );
                            if i == 0 {
                                peaks.move_to(peak_origin);
                            } else {
                                peaks.line_to(peak_origin);
                            }
                            let peak_tint = peak_color(options, channel, index, band, accent);
                            let peak_height = if flags.leds {
                                geometry.led_height(f64::from(band.peaks[channel])) / pixel_ratio
                            } else {
                                graph_height * band.peaks[channel]
                            };
                            if band.peaks[channel] <= 0.
                                || peak_tint.a <= 0.
                                || (flags.leds
                                    && peak_height < geometry.leds.unwrap().2 / pixel_ratio)
                            {
                                continue;
                            }
                            projection.paint_quad(
                                fill(
                                    Bounds::new(
                                        bounds.origin
                                            + point(px(ox + x), px(analyzer_bottom - peak_height)),
                                        size(
                                            px(step.max(1.)),
                                            px(if flags.leds {
                                                geometry.leds.unwrap().3 / pixel_ratio
                                            } else {
                                                2. / pixel_ratio
                                            }),
                                        ),
                                    ),
                                    peak_tint,
                                ),
                                window,
                            );
                        }
                    }
                }
                if graph && !radial && number(options, "fillAlpha", 0.) > 0. {
                    if !radial {
                        area.line_to(point(
                            last_origin.x,
                            bounds.origin.y + px(oy + graph_height),
                        ));
                    }
                    area.close();
                    if let Ok(path) = area.build() {
                        paint_colored_path(
                            path,
                            Bounds::new(
                                bounds.origin
                                    + point(
                                        px(if radial { ox - copy as f32 * cw } else { ox }),
                                        px(oy),
                                    ),
                                size(
                                    px(if radial { channel_width } else { cw }),
                                    px(graph_height),
                                ),
                            ),
                            options,
                            (channel, reversed),
                            (
                                accent,
                                number(options, "fillAlpha", 0.) * number(options, "opacity", 1.),
                            ),
                            projection,
                            window,
                        );
                    }
                }
                if graph
                    && !radial
                    && line_width > 0.
                    && let Ok(path) = line.build()
                {
                    paint_colored_path(
                        path,
                        Bounds::new(
                            bounds.origin
                                + point(
                                    px(if radial { ox - copy as f32 * cw } else { ox }),
                                    px(oy),
                                ),
                            size(
                                px(if radial { channel_width } else { cw }),
                                px(graph_height),
                            ),
                        ),
                        options,
                        (channel, reversed),
                        (accent, number(options, "opacity", 1.)),
                        projection,
                        window,
                    );
                }
                if flags.peak_line
                    && !radial
                    && enabled(options, "showPeaks")
                    && let Ok(path) = peaks.build()
                {
                    paint_colored_path(
                        path,
                        Bounds::new(
                            bounds.origin
                                + point(
                                    px(if radial { ox - copy as f32 * cw } else { ox }),
                                    px(oy),
                                ),
                            size(
                                px(if radial { channel_width } else { cw }),
                                px(graph_height),
                            ),
                        ),
                        options,
                        (channel, reversed),
                        (accent, number(options, "opacity", 1.)),
                        projection,
                        window,
                    );
                }
            }
        }
    }
    if enabled(options, "showScaleX") && radial {
        let (diameter, scale_height) = visualizer::radial_scale_dimensions(canvas_size, options);
        let center_r = (diameter / 2.).floor();
        let center = bounds.origin
            + point(
                px(((canvas_size[0] / 2.).floor() + center_r - diameter / 2.) / pixel_ratio),
                px(((canvas_size[1] / 2.).floor() + center_r - diameter / 2.) / pixel_ratio),
            );
        let radius = (center_r - scale_height / 2.) / pixel_ratio;
        if radius > 0. {
            let mut ring = PathBuilder::stroke(px(scale_height / pixel_ratio));
            ring.move_to(radial_point(center, 0., radius));
            for angle in [std::f32::consts::PI, std::f32::consts::TAU] {
                ring.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    radial_point(center, angle, radius),
                );
            }
            if let Ok(ring) = ring.build() {
                window.paint_path(ring, rgba(0x000000cc));
            }
        }
        let font_size = px((scale_height / 2.).floor() / pixel_ratio);
        for label in visualizer::frequency_labels(options, canvas_size, scale_height, |_| 0.) {
            let angle = std::f32::consts::TAU * label.position / canvas_size[0]
                + number(options, "_spin", 0.);
            let anchor = radial_point(
                center,
                angle - std::f32::consts::FRAC_PI_2,
                (center_r - scale_height * 0.7) / pixel_ratio,
            );
            let tint: Hsla = rgb(if label.highlight { 0x44ff44 } else { 0xffffff }).into();
            let text: SharedString = label.text.into();
            let run = TextRun {
                letter_spacing_fraction: 0.,
                font_size: None,
                line_height: None,
                len: text.len(),
                font: font(cx.theme().font_family.clone()),
                color: tint,
                background_color: None,
                underline: None,
                overline: None,
                strikethrough: None,
                letter_spacing: gpui::px(0.),
                text_spacing_trim: gpui::TextSpacingTrim::Normal,
                text_direction: Default::default(),
            };
            let line = window
                .text_system()
                .shape_line(text, font_size, &[run], None);
            let transform = TransformationMatrix::unit()
                .translate(anchor.scale(window.scale_factor()))
                .rotate(radians(angle))
                .translate(point(-anchor.x, -anchor.y).scale(window.scale_factor()));
            for run in &line.runs {
                for glyph in &run.glyphs {
                    let _ = window.paint_glyph_transformed(
                        anchor + glyph.position - point(line.width / 2., px(0.)),
                        run.font_id,
                        glyph.id,
                        font_size,
                        tint,
                        transform,
                    );
                }
            }
        }
    } else if enabled(options, "showScaleX") {
        let scale_height =
            (20. * pixel_ratio).max((canvas_size[0].min(canvas_size[1]) / 32.).floor());
        let font_size = px((scale_height / 2.).floor() / pixel_ratio);
        let labels = visualizer::frequency_labels(options, canvas_size, scale_height, |text| {
            let text: SharedString = text.to_owned().into();
            let run = TextRun {
                letter_spacing_fraction: 0.,
                font_size: None,
                line_height: None,
                len: text.len(),
                font: font(cx.theme().font_family.clone()),
                color: rgb(0xffffff).into(),
                background_color: None,
                underline: None,
                overline: None,
                strikethrough: None,
                letter_spacing: gpui::px(0.),
                text_spacing_trim: gpui::TextSpacingTrim::Normal,
                text_direction: Default::default(),
            };
            f32::from(
                window
                    .text_system()
                    .shape_line(text, font_size, &[run], None)
                    .width,
            ) * pixel_ratio
        });
        let origin = bounds.origin + point(px(0.), px(height - scale_height / pixel_ratio));
        window.paint_quad(fill(
            Bounds::new(
                origin,
                size(bounds.size.width, px(scale_height / pixel_ratio)),
            ),
            rgba(0x000000cc),
        ));
        for label in labels {
            let text: SharedString = label.text.into();
            let run = TextRun {
                letter_spacing_fraction: 0.,
                font_size: None,
                line_height: None,
                len: text.len(),
                font: font(cx.theme().font_family.clone()),
                color: rgb(if label.highlight { 0x44ff44 } else { 0xffffff }).into(),
                background_color: None,
                underline: None,
                overline: None,
                strikethrough: None,
                letter_spacing: gpui::px(0.),
                text_spacing_trim: gpui::TextSpacingTrim::Normal,
                text_direction: Default::default(),
            };
            let mut line = window.text_system().shape_line(
                text.clone(),
                font_size,
                std::slice::from_ref(&run),
                None,
            );
            let width = px(label.max_width / pixel_ratio);
            if line.width > width {
                line = window.text_system().shape_line(
                    text,
                    font_size * (f32::from(width) / f32::from(line.width)),
                    &[run],
                    None,
                );
            }
            let _ = line.paint(
                origin
                    + point(
                        px(label.position / pixel_ratio) - line.width / 2.,
                        px(scale_height * 0.75 / pixel_ratio) - line.ascent,
                    ),
                line.ascent + line.descent,
                window,
                cx,
            );
        }
    }
    if enabled(options, "showFPS") {
        let scale_height =
            (20. * pixel_ratio).max((canvas_size[0].min(canvas_size[1]) / 32.).floor());
        let text: SharedString = format!("{:.0}", number(options, "_fps", 0.)).into();
        let run = TextRun {
            letter_spacing_fraction: 0.,
            font_size: None,
            line_height: None,
            len: text.len(),
            font: Font {
                weight: FontWeight::BOLD,
                ..font("sans-serif")
            },
            color: rgb(0x00ff00).into(),
            background_color: None,
            underline: None,
            overline: None,
            strikethrough: None,
            letter_spacing: gpui::px(0.),
            text_spacing_trim: gpui::TextSpacingTrim::Normal,
            text_direction: Default::default(),
        };
        let line =
            window
                .text_system()
                .shape_line(text, px(scale_height / pixel_ratio), &[run], None);
        let origin = point(
            bounds.right() - px(scale_height / pixel_ratio) - line.width,
            bounds.top() + px(scale_height * 2. / pixel_ratio) - line.ascent,
        );
        let _ = line.paint(origin, line.ascent + line.descent, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    #[gpui::test]
    fn nonzero_spectrum_frames_paint_original_polygon_and_reflection_cases(
        cx: &mut TestAppContext,
    ) {
        use std::{cell::Cell, rc::Rc};
        struct Probe {
            options: Value,
            painted: Rc<Cell<usize>>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let options = self.options.clone();
                let painted = self.painted.clone();
                div().size_full().child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, cx| {
                            let bands: Vec<_> = (0..16)
                                .map(|index| visualizer::Band {
                                    position: index as f32 / 16.,
                                    width: 1. / 16.,
                                    frequency: 20. * 2_f32.powf(index as f32 * 0.6),
                                    values: [0.25 + index as f64 / 24., 0.9 - index as f64 / 24.],
                                    peaks: [0.95, 0.95],
                                    peak_alpha: [0.7, 0.7],
                                    ..Default::default()
                                })
                                .collect();
                            paint_spectrum(
                                bounds,
                                &bands,
                                &options,
                                rgb(0x6688ff).into(),
                                window,
                                cx,
                            );
                            painted.set(painted.get() + 1);
                        },
                    )
                    .size_full(),
                )
            }
        }
        cx.update(gpui_component::init);
        let painted = Rc::new(Cell::new(0));
        let (probe, cx) = cx.add_window_view(|_, _| Probe {
            options: json!({}),
            painted: painted.clone(),
        });
        let reference: Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/spectrum-reference.json"
        ))
        .unwrap();
        let mut cases: Vec<Value> = ["polygons", "reflections"]
            .into_iter()
            .flat_map(|key| {
                reference[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|case| case["options"].clone())
            })
            .collect();
        for mode in [1, 3, 8, 10] {
            cases.push(json!({"mode": mode, "ledBars": true, "trueLeds": true, "channelLayout": "dual-vertical", "reflexRatio": 0.5, "showPeaks": true, "gradient": "rainbow"}));
        }
        for layout in [
            "single",
            "dual-horizontal",
            "dual-vertical",
            "dual-combined",
        ] {
            for mirror in [-1, 0, 1] {
                for inverted in [false, true] {
                    cases.push(json!({"mode": 10, "radial": true, "radialInvert": inverted, "channelLayout": layout, "mirror": mirror, "peakLine": true, "gradient": "prism"}));
                }
            }
        }
        for mut options in cases {
            options["_pixelRatio"] = json!(2.);
            options["_bandCount"] = json!(16);
            options["showScaleX"] = json!(true);
            options["showScaleY"] = json!(true);
            options["showPeaks"] = json!(true);
            options["fillAlpha"] = json!(0.5);
            let before = painted.get();
            probe.update(cx, |probe, cx| {
                probe.options = options;
                cx.notify();
            });
            cx.refresh().unwrap();
            cx.run_until_parked();
            assert!(painted.get() > before);
        }
    }
    #[::core::prelude::v1::test]
    fn original_radial_polygon_vertices_and_reflection_transforms() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/spectrum-reference.json"
        ))
        .unwrap();
        for case in reference["polygons"].as_array().unwrap() {
            let options = &case["options"];
            let layout = options["channelLayout"].as_str().unwrap();
            let geometry = visualizer::spectrum_geometry([641., 481.], options, 1.);
            let count = case["count"].as_u64().unwrap() as usize;
            let bands: Vec<_> = (0..count)
                .map(|index| visualizer::Band {
                    position: index as f32 / count as f32,
                    width: 1. / count as f32,
                    ..Default::default()
                })
                .collect();
            let (x, width) = geometry.bar_bounds(&bands, options)[0];
            let mirror = number(options, "mirror", 0.);
            for (channel, expected) in case["paths"].as_array().unwrap().iter().enumerate() {
                let (radius, reach) = visualizer::radial_dimensions([641., 481.], options, channel);
                let end = radius + (reach * case["value"].as_f64().unwrap() as f32).trunc();
                let copies = if mirror != 0. && layout != "dual-horizontal" {
                    2
                } else {
                    1
                };
                let mut vertices = vec![];
                for copy in 0..copies {
                    let angles = [x, x + width].map(|x| {
                        radial_angle(
                            x / geometry.width,
                            layout,
                            channel,
                            mirror,
                            copy,
                            0.,
                            [641., geometry.width],
                        )
                    });
                    for (angle, radius) in [
                        (angles[0], radius),
                        (angles[0], end),
                        (angles[1], end),
                        (angles[1], radius),
                    ] {
                        vertices.push(radial_point(point(px(320.), px(240.)), angle, radius));
                    }
                    if enabled(options, "roundBars") {
                        vertices.push(radial_point(point(px(320.), px(240.)), angles[0], radius));
                    }
                    assert!(
                        radial_bar_path(
                            point(px(320.), px(240.)),
                            angles,
                            [radius, end],
                            enabled(options, "roundBars"),
                            None
                        )
                        .build()
                        .is_ok()
                    );
                }
                assert_eq!(vertices.len(), expected.as_array().unwrap().len(), "{case}");
                for (actual, expected) in vertices.into_iter().zip(expected.as_array().unwrap()) {
                    assert!(
                        (f32::from(actual.x) - expected[0].as_f64().unwrap() as f32).abs() < 0.0002
                            && (f32::from(actual.y) - expected[1].as_f64().unwrap() as f32).abs()
                                < 0.0002,
                        "{actual:?} {expected} {case}"
                    );
                }
            }
        }
        for case in reference["reflections"].as_array().unwrap() {
            let options = &case["options"];
            let vertical = options["channelLayout"] == "dual-vertical";
            let geometry = visualizer::spectrum_geometry([641., 481.], options, 1.);
            for (channel, draw) in case["draws"].as_array().unwrap().iter().enumerate() {
                let top = if vertical {
                    (geometry.channel_height + geometry.channel_gap) * channel as f32
                } else {
                    0.
                };
                let height = geometry.height;
                let projection = SpectrumProjection {
                    source_bottom: top + height,
                    scale: if enabled(options, "reflexFit") || vertical {
                        (geometry.channel_height - height) / height
                    } else {
                        1.
                    },
                    reflected: true,
                    clip: Bounds::new(
                        point(px(0.), px(top + height)),
                        size(px(641.), px(geometry.channel_height - height)),
                    ),
                };
                let actual = projection.bounds(Bounds::new(
                    point(px(0.), px(top)),
                    size(px(641.), px(height)),
                ));
                let rectangle = draw["rectangle"].as_array().unwrap();
                let y = rectangle[5].as_f64().unwrap() as f32;
                let expected_height = rectangle[7].as_f64().unwrap() as f32;
                assert!(
                    (f32::from(actual.origin.y) - (481. - y - expected_height)).abs() < 0.0001,
                    "{case}"
                );
                assert!(
                    (f32::from(actual.size.height) - expected_height).abs() < 0.0001,
                    "{case}"
                );
                assert_eq!(draw["alpha"], json!(0.4));
                assert_eq!(draw["filter"], json!("brightness(0.7)"));
            }
        }
    }
    #[::core::prelude::v1::test]
    fn original_canvas_gradient_fields_match_stereo_radial_and_luminance_layouts() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/spectrum-reference.json"
        ))
        .unwrap();
        let cases = reference["gradientFields"].as_array().unwrap();
        assert_eq!(cases.len(), 1680);
        for case in cases {
            let mut options = case["options"].clone();
            options["_canvasSize"] = case["size"].clone();
            let actual = spectrum_color(
                &options,
                case["channel"].as_u64().unwrap() as usize,
                case["index"].as_f64().unwrap() as f32,
                case["level"].as_f64().unwrap(),
                rgb(0).into(),
            )
            .to_rgb();
            for (channel, actual) in [actual.r, actual.g, actual.b].into_iter().enumerate() {
                let expected = case["expected"][channel].as_f64().unwrap() as f32;
                assert!(
                    (actual - expected).abs() < 0.00001,
                    "{case}: {actual} != {expected}"
                );
            }
        }
    }
    #[::core::prelude::v1::test]
    fn original_analyzer_color_cycles_and_level_thresholds() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/spectrum-reference.json"
        ))
        .unwrap();
        assert_eq!(&reference["gradients"], visualizer::spectrum_gradients());
        for case in reference["colors"].as_array().unwrap() {
            let options = json!({"mode": 3, "gradient": case["name"], "colorMode": case["colorMode"], "_bandCount": 16});
            let actual = color(
                &options,
                0,
                case["index"].as_f64().unwrap() as f32 / 15.,
                case["level"].as_f64().unwrap(),
                rgb(0).into(),
            )
            .to_rgb();
            let expected =
                feishin_core::appearance::color(case["expected"].as_str().unwrap()).unwrap();
            assert!(
                (actual.r - expected[0]).abs() < 0.00001
                    && (actual.g - expected[1]).abs() < 0.00001
                    && (actual.b - expected[2]).abs() < 0.00001,
                "{case}"
            );
        }
        let options = json!({"mode": 3, "gradient": "custom", "colorMode": "bar-level", "customGradients": [{"name": "custom", "colorStops": [{"color": "red", "level": 0.3, "levelEnabled": true}, {"color": "green", "level": 0.9, "levelEnabled": true}, {"color": "blue", "level": 0.6, "levelEnabled": true}]}]});
        assert_eq!(
            color(&options, 0, 0., 0.5, rgb(0).into()).to_rgb(),
            rgb(0x0000ff)
        );
        let horizontal = json!({"mode": 10, "gradient": "custom", "colorMode": "bar-index", "customGradients": [{"name": "custom", "dir": "h", "colorStops": ["red", "blue"]}]});
        assert_eq!(
            color(&horizontal, 0, 0., 0.5, rgb(0).into()).to_rgb(),
            rgb(0xff0000)
        );
        assert_eq!(
            color(&horizontal, 0, 1., 0.5, rgb(0).into()).to_rgb(),
            rgb(0x0000ff)
        );
    }
    #[::core::prelude::v1::test]
    fn radial_channels_share_a_center_and_use_opposite_half_circles() {
        use std::f32::consts::{FRAC_PI_2, PI};
        assert!(
            (radial_angle(0., "dual-horizontal", 0, 0., 0, 0., [1280., 640.]) - FRAC_PI_2).abs()
                < 0.000001
        );
        assert!(
            (radial_angle(0., "dual-horizontal", 1, 0., 0, 0., [1280., 640.]) + FRAC_PI_2).abs()
                < 0.000001
        );
        let left = radial_angle(0.5, "dual-horizontal", 0, 0., 0, 0., [1280., 640.]);
        let right = radial_angle(0.5, "dual-horizontal", 1, 0., 0, 0., [1280., 640.]);
        assert!((left - PI).abs() < 0.000001 && right.abs() < 0.000001);
        for layout in ["single", "dual-vertical", "dual-combined"] {
            assert_eq!(
                radial_angle(0.25, layout, 0, 0., 0, 0., [1280., 640.]),
                radial_angle(0.25, layout, 1, 0., 0, 0., [1280., 640.])
            );
            let first = radial_angle(0.5, layout, 0, 1., 0, 0., [1280., 640.]);
            let second = radial_angle(0.5, layout, 0, 1., 1, 0., [1280., 640.]);
            assert!((first + second + PI).abs() < 0.000001);
        }
    }
    #[::core::prelude::v1::test]
    fn retained_radial_peaks_and_intermediate_gradient_stops_are_independent_of_live_level() {
        let mut band = visualizer::Band {
            values: [0.1, 0.1],
            peaks: [0.8, 0.8],
            peak_alpha: [0.4, 0.4],
            ..Default::default()
        };
        let mut peak_options =
            json!({"mode": 3, "gradient": "orangered", "alphaBars": true, "opacity": 0.5});
        assert!((peak_color(&peak_options, 0, 0., &band, rgb(0).into()).a - 0.4).abs() < 0.000001);
        band.values = [0., 0.];
        assert!((peak_color(&peak_options, 0, 0., &band, rgb(0).into()).a - 0.4).abs() < 0.000001);
        peak_options["fadePeaks"] = json!(true);
        assert!((peak_color(&peak_options, 0, 0., &band, rgb(0).into()).a - 0.2).abs() < 0.000001);
        assert_eq!(radial_level(50., 25., 0., false), 50.);
        assert_eq!(radial_level(50., 25., 0.8, false), 70.);
        assert_eq!(radial_level(50., 25., 0.8, true), 30.);
        assert_eq!(radial_level(5., 25., 1., true), 0.);
        let options = json!({"gradient": "test", "customGradients": [{"name": "test", "dir": "v", "colorStops": [{"color": "#ff0000", "pos": 0., "positionEnabled": true}, {"color": "#00ff00", "pos": 0.5, "positionEnabled": true}, {"color": "#0000ff", "pos": 1., "positionEnabled": true}]}]});
        let middle = color(&options, 0, 0.2, 0.5, rgb(0).into()).to_rgb();
        assert!(middle.g > 0.99 && middle.r < 0.01 && middle.b < 0.01);
        let lower = color(&options, 0, 0.2, 0.25, rgb(0).into()).to_rgb();
        assert!((lower.g - 0.5).abs() < 0.01 && (lower.b - 0.5).abs() < 0.01);
        assert!(!horizontal_gradient(&options, 0));
        assert!(horizontal_gradient(&json!({"gradient": "rainbow"}), 0));
    }
}
