use eframe::egui;
use std::sync::{Arc, Mutex};

use crate::audio::engine::{AudioEngine, AudioState};
use crate::audio::export;
use crate::audio::filters::{AudioFilter, FilterType};
use crate::audio::oscillator::{Oscillator, Waveform};
use crate::audio::spectrum;
use crate::audio::therapy::{NotchFilter, SweepMode};
use crate::presets::{
    self, AmpModPreset, BinauralPreset, FilterPreset, FractalTonePreset, NotchPreset,
    OscillatorPreset, Preset, ResidualInhibitionPreset, SweepPreset,
};

/// Which panel tab is active.
#[derive(PartialEq)]
enum Tab {
    Oscillators,
    Sweep,
    Binaural,
    NotchFilter,
    Filters,
    Therapy,
    Presets,
}

pub struct TinnitusApp {
    engine: Option<AudioEngine>,
    engine_error: Option<String>,
    active_tab: Tab,
    // For adding new oscillators
    new_osc_freq: f32,
    new_osc_waveform: Waveform,
    // For adding new notch filters
    new_notch_freq: f32,
    new_notch_bw: f32,
    // For adding new general filters
    new_filter_type: FilterType,
    new_filter_freq: f32,
    // Preset management
    preset_name: String,
    saved_presets: Vec<(String, std::path::PathBuf)>,
    status_message: Option<(String, std::time::Instant)>,
    // Waveform scope
    show_scope: bool,
    scope_samples: usize,
    // Spectrum analyzer
    show_spectrum: bool,
    spectrum_floor_db: f32,
}

impl TinnitusApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let engine = match AudioEngine::new() {
            Ok(e) => Some(e),
            Err(err) => {
                eprintln!("Audio engine error: {}", err);
                return Self {
                    engine: None,
                    engine_error: Some(err),
                    active_tab: Tab::Oscillators,
                    new_osc_freq: 1000.0,
                    new_osc_waveform: Waveform::Sine,
                    new_notch_freq: 4000.0,
                    new_notch_bw: 500.0,
                    new_filter_type: FilterType::LowPass,
                    new_filter_freq: 1000.0,
                    preset_name: String::new(),
                    saved_presets: presets::list_presets(),
                    status_message: None,
                    show_scope: false,
                    scope_samples: 1024,
                    show_spectrum: false,
                    spectrum_floor_db: -80.0,
                };
            }
        };

        Self {
            engine,
            engine_error: None,
            active_tab: Tab::Oscillators,
            new_osc_freq: 1000.0,
            new_osc_waveform: Waveform::Sine,
            new_notch_freq: 4000.0,
            new_notch_bw: 500.0,
            new_filter_type: FilterType::LowPass,
            new_filter_freq: 1000.0,
            preset_name: String::new(),
            saved_presets: presets::list_presets(),
            status_message: None,
            show_scope: false,
            scope_samples: 1024,
            show_spectrum: false,
            spectrum_floor_db: -80.0,
        }
    }

    fn state(&self) -> Option<&Arc<Mutex<AudioState>>> {
        self.engine.as_ref().map(|e| &e.state)
    }

    fn set_status(&mut self, msg: &str) {
        self.status_message = Some((msg.to_string(), std::time::Instant::now()));
    }

    fn show_transport(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };
        let mut state = state_arc.lock().unwrap();

        ui.horizontal(|ui| {
            let play_text = if state.playing {
                "\u{23F8}  Pause"
            } else {
                "\u{25B6}  Play"
            };
            if ui.button(play_text).clicked() {
                state.playing = !state.playing;
            }

            ui.separator();

            ui.label("Master Volume:");
            ui.add(egui::Slider::new(&mut state.master_volume, 0.0..=1.0).show_value(true));

            // Scope toggle
            let scope_label = if self.show_scope {
                "\u{1F4C9} Hide Scope"
            } else {
                "\u{1F4C9} Scope"
            };
            if ui.button(scope_label).clicked() {
                self.show_scope = !self.show_scope;
            }

            // Spectrum toggle
            let spectrum_label = if self.show_spectrum {
                "\u{1F4CA} Hide Spectrum"
            } else {
                "\u{1F4CA} Spectrum"
            };
            if ui.button(spectrum_label).clicked() {
                self.show_spectrum = !self.show_spectrum;
            }

            if ui.button("\u{1F4E5} Export WAV").clicked() {
                // Open native save dialog on a separate thread to avoid blocking
                let state_arc_clone = state_arc.clone();
                std::thread::spawn(move || {
                    let save_path = rfd::FileDialog::new()
                        .set_title("Export 1-Minute WAV")
                        .add_filter("WAV Audio", &["wav"])
                        .set_file_name("tinnitus_therapy.wav")
                        .save_file();

                    if let Some(path) = save_path {
                        let state = state_arc_clone.lock().unwrap();
                        let result = export::export_wav(&state, &path, 60.0);
                        drop(state);
                        match result {
                            Ok(_) => eprintln!("Exported WAV to {:?}", path),
                            Err(e) => eprintln!("Export error: {}", e),
                        }
                    }
                });
                self.set_status("Exporting 1-minute WAV...");
            }

            if let Some(ref engine) = self.engine {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "\u{1F50A} {} | {} Hz",
                            engine.device_name, state.sample_rate as u32
                        ))
                        .small()
                        .weak(),
                    );
                });
            }
        });
    }

    fn show_oscillators_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };

        // Add new oscillator controls
        ui.group(|ui| {
            ui.label(egui::RichText::new("Add Tone").strong());
            ui.horizontal(|ui| {
                ui.label("Frequency (Hz):");
                ui.add(
                    egui::DragValue::new(&mut self.new_osc_freq)
                        .speed(1.0)
                        .range(20.0..=20000.0)
                        .suffix(" Hz"),
                );

                ui.label("Waveform:");
                egui::ComboBox::from_id_salt("new_waveform")
                    .selected_text(self.new_osc_waveform.name())
                    .show_ui(ui, |ui| {
                        for wf in Waveform::ALL {
                            ui.selectable_value(&mut self.new_osc_waveform, *wf, wf.name());
                        }
                    });

                if ui.button("\u{2795} Add").clicked() {
                    let mut state = state_arc.lock().unwrap();
                    state
                        .oscillators
                        .push(Oscillator::new(self.new_osc_waveform, self.new_osc_freq));
                }
            });
        });

        ui.add_space(8.0);

        // List existing oscillators
        let mut state = state_arc.lock().unwrap();
        let mut to_remove: Option<usize> = None;

        if state.oscillators.is_empty() {
            ui.label("No tones added yet. Use the controls above to add one.");
            return;
        }

        for (i, osc) in state.oscillators.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut osc.enabled, "");
                    ui.label(
                        egui::RichText::new(format!(
                            "Tone {} - {}",
                            i + 1,
                            osc.waveform.name()
                        ))
                        .strong(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("\u{1F5D1}").clicked() {
                            to_remove = Some(i);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Freq:");
                    ui.add(
                        egui::DragValue::new(&mut osc.frequency)
                            .speed(1.0)
                            .range(20.0..=20000.0)
                            .suffix(" Hz"),
                    );

                    for &freq in &[250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0] {
                        if ui.small_button(format!("{}", freq as u32)).clicked() {
                            osc.frequency = freq;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Volume:");
                    ui.add(egui::Slider::new(&mut osc.volume, 0.0..=1.0));
                });

                ui.horizontal(|ui| {
                    ui.label("Pan:");
                    ui.add(egui::Slider::new(&mut osc.pan, -1.0..=1.0).text("L/R"));
                });

                ui.horizontal(|ui| {
                    ui.label("Waveform:");
                    for wf in Waveform::ALL {
                        ui.selectable_value(&mut osc.waveform, *wf, wf.name());
                    }
                });
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_remove {
            state.oscillators.remove(idx);
        }
    }

    fn show_sweep_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };
        let mut state = state_arc.lock().unwrap();
        let sweep = &mut state.sweep;

        ui.label(egui::RichText::new("Frequency Sweep").strong().size(16.0));
        ui.label("Sweeps through a range of frequencies over time. Useful for finding your tinnitus frequency.");

        ui.add_space(8.0);

        ui.checkbox(&mut sweep.enabled, "Enable Sweep");

        ui.horizontal(|ui| {
            ui.label("Start Frequency:");
            ui.add(
                egui::DragValue::new(&mut sweep.start_freq)
                    .speed(10.0)
                    .range(20.0..=20000.0)
                    .suffix(" Hz"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("End Frequency:");
            ui.add(
                egui::DragValue::new(&mut sweep.end_freq)
                    .speed(10.0)
                    .range(20.0..=20000.0)
                    .suffix(" Hz"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Duration:");
            ui.add(
                egui::DragValue::new(&mut sweep.duration_secs)
                    .speed(0.5)
                    .range(1.0..=120.0)
                    .suffix(" sec"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Mode:");
            ui.selectable_value(&mut sweep.mode, SweepMode::Linear, "Linear");
            ui.selectable_value(&mut sweep.mode, SweepMode::Logarithmic, "Logarithmic");
        });

        ui.horizontal(|ui| {
            ui.label("Volume:");
            ui.add(egui::Slider::new(&mut sweep.volume, 0.0..=1.0));
        });

        ui.checkbox(&mut sweep.looping, "Loop");

        if sweep.enabled {
            let progress = sweep.progress();
            let current_freq = sweep.current_frequency();
            ui.add_space(4.0);
            ui.add(egui::ProgressBar::new(progress).text(format!("{:.0} Hz", current_freq)));

            if ui.button("Reset Sweep").clicked() {
                sweep.reset();
            }
        }
    }

    fn show_binaural_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };
        let mut state = state_arc.lock().unwrap();
        let binaural = &mut state.binaural;

        ui.label(egui::RichText::new("Binaural Beats").strong().size(16.0));
        ui.label("Plays slightly different frequencies in each ear to create a perceived beat. Use headphones for best effect.");

        ui.add_space(8.0);

        ui.checkbox(&mut binaural.enabled, "Enable Binaural Beats");

        ui.horizontal(|ui| {
            ui.label("Base Frequency:");
            ui.add(
                egui::DragValue::new(&mut binaural.base_freq)
                    .speed(1.0)
                    .range(50.0..=1000.0)
                    .suffix(" Hz"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Beat Frequency:");
            ui.add(
                egui::DragValue::new(&mut binaural.beat_freq)
                    .speed(0.1)
                    .range(0.5..=40.0)
                    .suffix(" Hz"),
            );
        });

        ui.add_space(4.0);
        ui.label("Brainwave Presets:");
        ui.horizontal(|ui| {
            if ui.button("Delta (2 Hz) - Deep Sleep").clicked() {
                binaural.beat_freq = 2.0;
            }
            if ui.button("Theta (6 Hz) - Meditation").clicked() {
                binaural.beat_freq = 6.0;
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Alpha (10 Hz) - Relaxation").clicked() {
                binaural.beat_freq = 10.0;
            }
            if ui.button("Beta (20 Hz) - Focus").clicked() {
                binaural.beat_freq = 20.0;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Volume:");
            ui.add(egui::Slider::new(&mut binaural.volume, 0.0..=1.0));
        });

        if binaural.enabled {
            ui.add_space(4.0);
            ui.label(format!(
                "Left ear: {:.1} Hz  |  Right ear: {:.1} Hz",
                binaural.left_freq(),
                binaural.right_freq()
            ));
        }
    }

    fn show_notch_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };

        ui.label(egui::RichText::new("Notch Filters").strong().size(16.0));
        ui.label("Add one or more notch filters to remove specific frequency bands. Set each center to a tinnitus frequency for notched sound therapy.");

        ui.add_space(8.0);

        // Add new notch filter
        ui.group(|ui| {
            ui.label(egui::RichText::new("Add Notch Filter").strong());
            ui.horizontal(|ui| {
                ui.label("Center Freq:");
                ui.add(
                    egui::DragValue::new(&mut self.new_notch_freq)
                        .speed(10.0)
                        .range(50.0..=16000.0)
                        .suffix(" Hz"),
                );
                ui.label("Bandwidth:");
                ui.add(
                    egui::DragValue::new(&mut self.new_notch_bw)
                        .speed(5.0)
                        .range(50.0..=5000.0)
                        .suffix(" Hz"),
                );
                if ui.button("\u{2795} Add").clicked() {
                    let mut state = state_arc.lock().unwrap();
                    let sr = state.sample_rate;
                    let mut nf = NotchFilter::new(self.new_notch_freq, self.new_notch_bw);
                    nf.enabled = true;
                    nf.compute_coefficients(sr);
                    state.notch_filters.push(nf);
                }
            });
        });

        ui.add_space(8.0);

        let mut state = state_arc.lock().unwrap();
        let sample_rate = state.sample_rate;
        let mut to_remove: Option<usize> = None;

        if state.notch_filters.is_empty() {
            ui.label("No notch filters added yet.");
            return;
        }

        for (i, notch) in state.notch_filters.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut notch.enabled, "");
                    ui.label(
                        egui::RichText::new(format!(
                            "Notch {} - {:.0} Hz",
                            i + 1,
                            notch.center_freq
                        ))
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("\u{1F5D1}").clicked() {
                            to_remove = Some(i);
                        }
                    });
                });

                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label("Center Frequency:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut notch.center_freq)
                                .speed(10.0)
                                .range(50.0..=16000.0)
                                .suffix(" Hz"),
                        )
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Bandwidth:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut notch.bandwidth)
                                .speed(5.0)
                                .range(50.0..=5000.0)
                                .suffix(" Hz"),
                        )
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Depth:");
                    ui.add(
                        egui::Slider::new(&mut notch.depth, 0.0..=1.0)
                            .text("0=Full notch  1=Bypass"),
                    );
                });

                if changed {
                    notch.compute_coefficients(sample_rate);
                }
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_remove {
            state.notch_filters.remove(idx);
        }
    }

    fn show_filters_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };

        ui.label(egui::RichText::new("Audio Filters").strong().size(16.0));
        ui.label("Add filters to shape the overall sound. Filters are applied in series after oscillators and notch filters.");

        ui.add_space(8.0);

        // Add new filter
        ui.group(|ui| {
            ui.label(egui::RichText::new("Add Filter").strong());
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("new_filter_type")
                    .selected_text(self.new_filter_type.name())
                    .show_ui(ui, |ui| {
                        for ft in FilterType::ALL {
                            ui.selectable_value(&mut self.new_filter_type, *ft, ft.name());
                        }
                    });

                ui.label("Freq:");
                ui.add(
                    egui::DragValue::new(&mut self.new_filter_freq)
                        .speed(10.0)
                        .range(20.0..=20000.0)
                        .suffix(" Hz"),
                );

                if ui.button("\u{2795} Add").clicked() {
                    let mut state = state_arc.lock().unwrap();
                    let sr = state.sample_rate;
                    let mut af = AudioFilter::new(self.new_filter_type, self.new_filter_freq);
                    af.enabled = true;
                    af.compute_coefficients(sr);
                    state.filters.push(af);
                }
            });
        });

        ui.add_space(8.0);

        let mut state = state_arc.lock().unwrap();
        let sample_rate = state.sample_rate;
        let mut to_remove: Option<usize> = None;

        if state.filters.is_empty() {
            ui.label("No audio filters added yet.");
            return;
        }

        for (i, filter) in state.filters.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut filter.enabled, "");
                    ui.label(
                        egui::RichText::new(format!(
                            "Filter {} - {} @ {:.0} Hz",
                            i + 1,
                            filter.filter_type.name(),
                            filter.frequency
                        ))
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("\u{1F5D1}").clicked() {
                            to_remove = Some(i);
                        }
                    });
                });

                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label("Frequency:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut filter.frequency)
                                .speed(10.0)
                                .range(20.0..=20000.0)
                                .suffix(" Hz"),
                        )
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Q:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut filter.q)
                                .speed(0.01)
                                .range(0.1..=20.0),
                        )
                        .changed();
                });

                // Show gain for filter types that use it
                if matches!(
                    filter.filter_type,
                    FilterType::PeakingEQ | FilterType::LowShelf | FilterType::HighShelf
                ) {
                    ui.horizontal(|ui| {
                        ui.label("Gain:");
                        changed |= ui
                            .add(
                                egui::Slider::new(&mut filter.gain_db, -24.0..=24.0)
                                    .suffix(" dB"),
                            )
                            .changed();
                    });
                }

                ui.horizontal(|ui| {
                    ui.label("Mix:");
                    ui.add(
                        egui::Slider::new(&mut filter.mix, 0.0..=1.0)
                            .text("0=Dry  1=Wet"),
                    );
                });

                if changed {
                    filter.compute_coefficients(sample_rate);
                }
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_remove {
            state.filters.remove(idx);
        }
    }

    fn show_therapy_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };
        let mut state = state_arc.lock().unwrap();

        ui.label(egui::RichText::new("Therapy Techniques").strong().size(16.0));
        ui.label("Advanced sound therapy tools for tinnitus management.");

        ui.add_space(8.0);

        // Amplitude Modulation section
        ui.group(|ui| {
            ui.label(egui::RichText::new("Amplitude Modulation").strong());
            ui.label("Rhythmically varies the volume of the output signal.");
            ui.add_space(4.0);

            ui.checkbox(&mut state.amp_mod.enabled, "Enable AM");

            ui.horizontal(|ui| {
                ui.label("Rate:");
                ui.add(
                    egui::DragValue::new(&mut state.amp_mod.rate)
                        .speed(0.1)
                        .range(0.1..=30.0)
                        .suffix(" Hz"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Depth:");
                ui.add(
                    egui::Slider::new(&mut state.amp_mod.depth, 0.0..=1.0)
                        .text("0=None  1=Full"),
                );
            });
        });

        ui.add_space(8.0);

        // Residual Inhibition section
        ui.group(|ui| {
            ui.label(egui::RichText::new("Residual Inhibition").strong());
            ui.label("Plays tonal bursts followed by silence to temporarily suppress tinnitus perception.");
            ui.add_space(4.0);

            ui.checkbox(&mut state.residual_inhibition.enabled, "Enable RI");

            ui.horizontal(|ui| {
                ui.label("Burst Freq:");
                ui.add(
                    egui::DragValue::new(&mut state.residual_inhibition.burst_freq)
                        .speed(10.0)
                        .range(100.0..=16000.0)
                        .suffix(" Hz"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Burst Duration:");
                ui.add(
                    egui::DragValue::new(&mut state.residual_inhibition.burst_duration)
                        .speed(0.05)
                        .range(0.1..=5.0)
                        .suffix(" sec"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Silence Duration:");
                ui.add(
                    egui::DragValue::new(&mut state.residual_inhibition.silence_duration)
                        .speed(0.05)
                        .range(0.1..=10.0)
                        .suffix(" sec"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(egui::Slider::new(
                    &mut state.residual_inhibition.burst_volume,
                    0.0..=1.0,
                ));
            });

            if state.residual_inhibition.enabled {
                let status = if state.residual_inhibition.is_in_burst() {
                    "BURST"
                } else {
                    "SILENCE"
                };
                ui.label(format!(
                    "Status: {} | Progress: {:.0}%",
                    status,
                    state.residual_inhibition.progress() * 100.0
                ));
            }
        });

        ui.add_space(8.0);

        // Fractal Tones section
        ui.group(|ui| {
            ui.label(egui::RichText::new("Fractal Tones").strong());
            ui.label("Generates evolving melodic tones using pentatonic scales — a random walk through musical space for relaxation.");
            ui.add_space(4.0);

            ui.checkbox(&mut state.fractal_tones.enabled, "Enable Fractal Tones");

            ui.horizontal(|ui| {
                ui.label("Base Freq:");
                ui.add(
                    egui::DragValue::new(&mut state.fractal_tones.base_freq)
                        .speed(1.0)
                        .range(100.0..=2000.0)
                        .suffix(" Hz"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Tempo:");
                ui.add(
                    egui::DragValue::new(&mut state.fractal_tones.tempo)
                        .speed(0.1)
                        .range(0.5..=10.0)
                        .suffix(" notes/sec"),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(egui::Slider::new(
                    &mut state.fractal_tones.volume,
                    0.0..=1.0,
                ));
            });
        });
    }

    fn show_presets_tab(&mut self, ui: &mut egui::Ui) {
        let Some(state_arc) = self.state().cloned() else {
            return;
        };

        ui.label(egui::RichText::new("Presets").strong().size(16.0));

        // Save current state as preset
        ui.group(|ui| {
            ui.label("Save Current Settings");
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.preset_name);
                if ui.button("Save").clicked() && !self.preset_name.is_empty() {
                    let state = state_arc.lock().unwrap();
                    let preset = state_to_preset(&state, &self.preset_name);
                    match presets::save_preset(&preset) {
                        Ok(_) => {
                            self.set_status(&format!("Saved preset '{}'", self.preset_name));
                            self.saved_presets = presets::list_presets();
                        }
                        Err(e) => self.set_status(&format!("Error: {}", e)),
                    }
                }
            });
        });

        ui.add_space(8.0);

        // Built-in presets
        ui.label(egui::RichText::new("Built-in Presets").strong());
        for preset in presets::builtin_presets() {
            ui.horizontal(|ui| {
                if ui.button(&preset.name).clicked() {
                    let mut state = state_arc.lock().unwrap();
                    apply_preset(&mut state, &preset);
                    self.set_status(&format!("Loaded '{}'", preset.name));
                }
            });
        }

        ui.add_space(8.0);

        // User-saved presets
        if !self.saved_presets.is_empty() {
            ui.label(egui::RichText::new("Your Presets").strong());
            let mut to_delete: Option<usize> = None;
            let mut loaded_name: Option<String> = None;

            let preset_entries: Vec<(usize, String, std::path::PathBuf)> = self
                .saved_presets
                .iter()
                .enumerate()
                .map(|(i, (name, path))| (i, name.clone(), path.clone()))
                .collect();

            for (i, name, path) in &preset_entries {
                ui.horizontal(|ui| {
                    if ui.button(name.as_str()).clicked() {
                        if let Ok(preset) = presets::load_preset(path) {
                            let mut state = state_arc.lock().unwrap();
                            apply_preset(&mut state, &preset);
                            loaded_name = Some(name.clone());
                        }
                    }
                    if ui.small_button("\u{1F5D1}").clicked() {
                        to_delete = Some(*i);
                    }
                });
            }

            if let Some(name) = loaded_name {
                self.set_status(&format!("Loaded '{}'", name));
            }
            if let Some(idx) = to_delete {
                let name = preset_entries[idx].1.clone();
                let path = &preset_entries[idx].2;
                if presets::delete_preset(path).is_ok() {
                    self.set_status(&format!("Deleted '{}'", name));
                    self.saved_presets = presets::list_presets();
                }
            }
        }
    }

    /// Draw the real-time waveform scope in a separate egui::Window.
    fn show_scope_window(&mut self, ctx: &egui::Context) {
        if !self.show_scope {
            return;
        }

        let Some(state_arc) = self.state().cloned() else {
            return;
        };

        let (left_samples, right_samples) = {
            let state = state_arc.lock().unwrap();
            state.waveform_buf.read_recent(self.scope_samples)
        };

        egui::Window::new("Waveform Scope")
            .open(&mut self.show_scope)
            .default_size([700.0, 380.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Samples:");
                    ui.add(
                        egui::Slider::new(&mut self.scope_samples, 128..=4096)
                            .logarithmic(true)
                            .text("visible"),
                    );
                });

                ui.add_space(4.0);

                // Compute layout: split remaining height evenly for L and R
                let available_h = ui.available_height();
                let label_h = 18.0;
                let spacing = 4.0;
                let channel_h = ((available_h - 2.0 * label_h - spacing) / 2.0).max(40.0);

                // Left channel
                ui.label(egui::RichText::new("Left Channel").strong().small());
                let (left_rect, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), channel_h), egui::Sense::hover());
                draw_waveform(ui, left_rect, &left_samples, egui::Color32::from_rgb(80, 200, 120));

                ui.add_space(spacing);

                // Right channel
                ui.label(egui::RichText::new("Right Channel").strong().small());
                let (right_rect, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), channel_h), egui::Sense::hover());
                draw_waveform(ui, right_rect, &right_samples, egui::Color32::from_rgb(100, 149, 237));
            });
    }

    /// Draw the real-time spectrum analyzer in a separate egui::Window.
    fn show_spectrum_window(&mut self, ctx: &egui::Context) {
        if !self.show_spectrum {
            return;
        }

        let Some(state_arc) = self.state().cloned() else {
            return;
        };

        let (buckets, sample_rate) = {
            let state = state_arc.lock().unwrap();
            let samples = state.read_spectrum_samples(4096);
            let sr = state.sample_rate;
            let b = spectrum::compute_spectrum_buckets(&samples, sr, 20, 20000.0, self.spectrum_floor_db);
            (b, sr)
        };
        let _ = sample_rate; // used for label accuracy

        egui::Window::new("Spectrum Analyzer")
            .open(&mut self.show_spectrum)
            .default_size([720.0, 340.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Noise floor:");
                    ui.add(
                        egui::Slider::new(&mut self.spectrum_floor_db, -120.0..=-20.0)
                            .suffix(" dB")
                            .text("floor"),
                    );
                });

                ui.add_space(4.0);

                let (bar_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ui.available_height()),
                    egui::Sense::hover(),
                );
                draw_spectrum(ui, bar_rect, &buckets, self.spectrum_floor_db);
            });
    }
}

// ──────────────────────────────────────────────────────────────
//  Drawing helpers
// ──────────────────────────────────────────────────────────────

/// Paint a waveform into a given rect.
/// The full [-1.0, 1.0] range maps exactly to the rect height.
fn draw_waveform(ui: &egui::Ui, rect: egui::Rect, samples: &[f32], color: egui::Color32) {
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 2.0, egui::Color32::from_gray(25));

    // Center line (0.0)
    let center_y = rect.center().y;
    painter.line_segment(
        [
            egui::pos2(rect.left(), center_y),
            egui::pos2(rect.right(), center_y),
        ],
        egui::Stroke::new(0.5, egui::Color32::from_gray(60)),
    );

    // +/- 0.5 grid lines
    let quarter = rect.height() / 4.0;
    for &offset in &[-quarter, quarter] {
        let y = center_y + offset;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(0.3, egui::Color32::from_gray(40)),
        );
    }

    if samples.is_empty() {
        return;
    }

    let n = samples.len();
    let w = rect.width();
    let half_h = rect.height() / 2.0;

    // Map sample value [-1, 1] → y pixel using the FULL rect height.
    // -1.0 → rect.bottom(), 0.0 → center, +1.0 → rect.top()
    let points: Vec<egui::Pos2> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let x = rect.left() + (i as f32 / (n - 1).max(1) as f32) * w;
            let y = center_y - s.clamp(-1.0, 1.0) * half_h;
            egui::pos2(x, y)
        })
        .collect();

    let stroke = egui::Stroke::new(1.2, color);
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], stroke);
    }

    // Scale labels
    let label_color = egui::Color32::from_gray(100);
    let font = egui::FontId::proportional(10.0);
    painter.text(
        egui::pos2(rect.left() + 2.0, rect.top() + 1.0),
        egui::Align2::LEFT_TOP,
        "+1.0",
        font.clone(),
        label_color,
    );
    painter.text(
        egui::pos2(rect.left() + 2.0, rect.bottom() - 1.0),
        egui::Align2::LEFT_BOTTOM,
        "-1.0",
        font,
        label_color,
    );
}

/// Paint a spectrum bar chart into a given rect.
/// `buckets` has 20 entries (0-1kHz, 1-2kHz, ... 19-20kHz), each in dB.
fn draw_spectrum(ui: &egui::Ui, rect: egui::Rect, buckets: &[f32], floor_db: f32) {
    let painter = ui.painter_at(rect);

    // Background
    painter.rect_filled(rect, 2.0, egui::Color32::from_gray(25));

    if buckets.is_empty() {
        return;
    }

    let num = buckets.len();
    let bar_spacing = 2.0;
    let total_spacing = bar_spacing * (num as f32 - 1.0);
    let label_area_h = 20.0; // reserved at bottom for frequency labels
    let db_label_area_w = 30.0; // reserved at left for dB labels
    let chart_left = rect.left() + db_label_area_w;
    let chart_bottom = rect.bottom() - label_area_h;
    let chart_top = rect.top() + 4.0;
    let chart_h = chart_bottom - chart_top;

    // Horizontal dB grid lines
    let label_color = egui::Color32::from_gray(100);
    let font = egui::FontId::proportional(9.0);
    let db_range = floor_db.abs();
    for &db in &[0.0, -20.0, -40.0, -60.0] {
        if db < floor_db {
            continue;
        }
        let norm = (db - floor_db) / db_range;
        let y = chart_bottom - norm * chart_h;
        painter.line_segment(
            [egui::pos2(chart_left, y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(0.3, egui::Color32::from_gray(40)),
        );
        painter.text(
            egui::pos2(rect.left() + 2.0, y),
            egui::Align2::LEFT_CENTER,
            format!("{:.0}", db),
            font.clone(),
            label_color,
        );
    }

    // Recalculate bar width for the chart area
    let chart_w = rect.right() - chart_left;
    let bar_w = ((chart_w - total_spacing) / num as f32).max(4.0);

    // Gradient colors for bars: green (low) → yellow → red (high)
    let bar_color = |normalized: f32| -> egui::Color32 {
        let r = (normalized * 2.0).min(1.0);
        let g = ((1.0 - normalized) * 2.0).min(1.0);
        egui::Color32::from_rgb((r * 200.0 + 55.0) as u8, (g * 200.0 + 55.0) as u8, 60)
    };

    for (i, &db_val) in buckets.iter().enumerate() {
        let x = chart_left + i as f32 * (bar_w + bar_spacing);

        // Normalize dB to 0..1 range (floor_db → 0.0, 0 dB → 1.0)
        let norm = ((db_val - floor_db) / db_range).clamp(0.0, 1.0);
        let bar_h = norm * chart_h;

        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(x, chart_bottom - bar_h),
            egui::vec2(bar_w, bar_h),
        );

        painter.rect_filled(bar_rect, 1.0, bar_color(norm));

        // Frequency label at bottom
        let freq_khz = (i + 1) as f32;
        // Only label every other bucket if space is tight
        if bar_w >= 20.0 || i % 2 == 0 {
            painter.text(
                egui::pos2(x + bar_w / 2.0, chart_bottom + 2.0),
                egui::Align2::CENTER_TOP,
                format!("{}k", freq_khz as u32),
                font.clone(),
                label_color,
            );
        }
    }
}

impl eframe::App for TinnitusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaint for live scope + sweep progress
        ctx.request_repaint();

        // Top panel: transport controls
        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            ui.add_space(4.0);
            if self.engine.is_some() {
                self.show_transport(ui);
            } else if let Some(ref err) = self.engine_error {
                ui.colored_label(egui::Color32::RED, format!("Audio Error: {}", err));
            }
            ui.add_space(4.0);
        });

        // Bottom panel: status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((ref msg, instant)) = self.status_message {
                    if instant.elapsed().as_secs() < 5 {
                        ui.label(msg);
                    }
                }
            });
        });

        // Floating analysis windows
        self.show_scope_window(ctx);
        self.show_spectrum_window(ctx);

        // Central panel with tabs
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Oscillators, "\u{1F3B5} Tones");
                ui.selectable_value(&mut self.active_tab, Tab::Sweep, "\u{1F4C8} Sweep");
                ui.selectable_value(&mut self.active_tab, Tab::Binaural, "\u{1F9E0} Binaural");
                ui.selectable_value(
                    &mut self.active_tab,
                    Tab::NotchFilter,
                    "\u{1F50C} Notch",
                );
                ui.selectable_value(&mut self.active_tab, Tab::Filters, "\u{1F527} Filters");
                ui.selectable_value(&mut self.active_tab, Tab::Therapy, "\u{2695} Therapy");
                ui.selectable_value(&mut self.active_tab, Tab::Presets, "\u{1F4BE} Presets");
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    Tab::Oscillators => self.show_oscillators_tab(ui),
                    Tab::Sweep => self.show_sweep_tab(ui),
                    Tab::Binaural => self.show_binaural_tab(ui),
                    Tab::NotchFilter => self.show_notch_tab(ui),
                    Tab::Filters => self.show_filters_tab(ui),
                    Tab::Therapy => self.show_therapy_tab(ui),
                    Tab::Presets => self.show_presets_tab(ui),
                }
            });
        });
    }
}

/// Snapshot the current AudioState into a Preset.
fn state_to_preset(state: &AudioState, name: &str) -> Preset {
    Preset {
        name: name.to_string(),
        oscillators: state
            .oscillators
            .iter()
            .map(|o| OscillatorPreset {
                waveform: o.waveform,
                frequency: o.frequency,
                volume: o.volume,
                enabled: o.enabled,
                pan: o.pan,
            })
            .collect(),
        sweep: SweepPreset {
            start_freq: state.sweep.start_freq,
            end_freq: state.sweep.end_freq,
            duration_secs: state.sweep.duration_secs,
            mode: state.sweep.mode,
            volume: state.sweep.volume,
            enabled: state.sweep.enabled,
            looping: state.sweep.looping,
        },
        binaural: BinauralPreset {
            base_freq: state.binaural.base_freq,
            beat_freq: state.binaural.beat_freq,
            volume: state.binaural.volume,
            enabled: state.binaural.enabled,
        },
        notch_filters: state
            .notch_filters
            .iter()
            .map(|nf| NotchPreset {
                center_freq: nf.center_freq,
                bandwidth: nf.bandwidth,
                enabled: nf.enabled,
                depth: nf.depth,
            })
            .collect(),
        filters: state
            .filters
            .iter()
            .map(|af| FilterPreset {
                filter_type: af.filter_type,
                frequency: af.frequency,
                q: af.q,
                gain_db: af.gain_db,
                enabled: af.enabled,
                mix: af.mix,
            })
            .collect(),
        amp_mod: Some(AmpModPreset {
            rate: state.amp_mod.rate,
            depth: state.amp_mod.depth,
            enabled: state.amp_mod.enabled,
        }),
        residual_inhibition: Some(ResidualInhibitionPreset {
            burst_freq: state.residual_inhibition.burst_freq,
            burst_duration: state.residual_inhibition.burst_duration,
            silence_duration: state.residual_inhibition.silence_duration,
            burst_volume: state.residual_inhibition.burst_volume,
            enabled: state.residual_inhibition.enabled,
        }),
        fractal_tones: Some(FractalTonePreset {
            base_freq: state.fractal_tones.base_freq,
            tempo: state.fractal_tones.tempo,
            volume: state.fractal_tones.volume,
            enabled: state.fractal_tones.enabled,
        }),
        master_volume: state.master_volume,
    }
}

/// Apply a Preset to the live AudioState.
fn apply_preset(state: &mut AudioState, preset: &Preset) {
    // Rebuild oscillators
    state.oscillators.clear();
    for op in &preset.oscillators {
        let mut osc = Oscillator::new(op.waveform, op.frequency);
        osc.volume = op.volume;
        osc.enabled = op.enabled;
        osc.pan = op.pan;
        state.oscillators.push(osc);
    }

    // Sweep
    state.sweep.start_freq = preset.sweep.start_freq;
    state.sweep.end_freq = preset.sweep.end_freq;
    state.sweep.duration_secs = preset.sweep.duration_secs;
    state.sweep.mode = preset.sweep.mode;
    state.sweep.volume = preset.sweep.volume;
    state.sweep.enabled = preset.sweep.enabled;
    state.sweep.looping = preset.sweep.looping;
    state.sweep.reset();

    // Binaural
    state.binaural.base_freq = preset.binaural.base_freq;
    state.binaural.beat_freq = preset.binaural.beat_freq;
    state.binaural.volume = preset.binaural.volume;
    state.binaural.enabled = preset.binaural.enabled;
    state.binaural.reset();

    // Notch filters — rebuild all
    state.notch_filters.clear();
    for np in &preset.notch_filters {
        let mut nf = NotchFilter::new(np.center_freq, np.bandwidth);
        nf.enabled = np.enabled;
        nf.depth = np.depth;
        nf.compute_coefficients(state.sample_rate);
        state.notch_filters.push(nf);
    }

    // General audio filters — rebuild all
    state.filters.clear();
    for fp in &preset.filters {
        let mut af = AudioFilter::new(fp.filter_type, fp.frequency);
        af.q = fp.q;
        af.gain_db = fp.gain_db;
        af.enabled = fp.enabled;
        af.mix = fp.mix;
        af.compute_coefficients(state.sample_rate);
        state.filters.push(af);
    }

    // Amplitude modulation
    if let Some(ref am) = preset.amp_mod {
        state.amp_mod.rate = am.rate;
        state.amp_mod.depth = am.depth;
        state.amp_mod.enabled = am.enabled;
        state.amp_mod.reset();
    }

    // Residual inhibition
    if let Some(ref ri) = preset.residual_inhibition {
        state.residual_inhibition.burst_freq = ri.burst_freq;
        state.residual_inhibition.burst_duration = ri.burst_duration;
        state.residual_inhibition.silence_duration = ri.silence_duration;
        state.residual_inhibition.burst_volume = ri.burst_volume;
        state.residual_inhibition.enabled = ri.enabled;
        state.residual_inhibition.reset();
    }

    // Fractal tones
    if let Some(ref ft) = preset.fractal_tones {
        state.fractal_tones.base_freq = ft.base_freq;
        state.fractal_tones.tempo = ft.tempo;
        state.fractal_tones.volume = ft.volume;
        state.fractal_tones.enabled = ft.enabled;
        state.fractal_tones.reset();
    }

    state.master_volume = preset.master_volume;
}
