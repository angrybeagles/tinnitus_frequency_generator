use crate::audio::filters::FilterType;
use crate::audio::oscillator::Waveform;
use crate::audio::therapy::SweepMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serializable snapshot of an oscillator's settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OscillatorPreset {
    pub waveform: Waveform,
    pub frequency: f32,
    pub volume: f32,
    pub enabled: bool,
    pub pan: f32,
}

/// Serializable snapshot of the frequency sweep settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SweepPreset {
    pub start_freq: f32,
    pub end_freq: f32,
    pub duration_secs: f32,
    pub mode: SweepMode,
    pub volume: f32,
    pub enabled: bool,
    pub looping: bool,
}

/// Serializable snapshot of binaural beat settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BinauralPreset {
    pub base_freq: f32,
    pub beat_freq: f32,
    pub volume: f32,
    pub enabled: bool,
}

/// Serializable snapshot of a single notch filter.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NotchPreset {
    pub center_freq: f32,
    pub bandwidth: f32,
    pub enabled: bool,
    pub depth: f32,
}

/// Serializable snapshot of a general audio filter.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FilterPreset {
    pub filter_type: FilterType,
    pub frequency: f32,
    pub q: f32,
    pub gain_db: f32,
    pub enabled: bool,
    pub mix: f32,
}

/// Serializable snapshot of amplitude modulation settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AmpModPreset {
    pub rate: f32,
    pub depth: f32,
    pub enabled: bool,
}

/// Serializable snapshot of residual inhibition settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ResidualInhibitionPreset {
    pub burst_freq: f32,
    pub burst_duration: f32,
    pub silence_duration: f32,
    pub burst_volume: f32,
    pub enabled: bool,
}

/// Serializable snapshot of fractal tone settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FractalTonePreset {
    pub base_freq: f32,
    pub tempo: f32,
    pub volume: f32,
    pub enabled: bool,
}

/// A complete preset containing all sound therapy settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Preset {
    pub name: String,
    pub oscillators: Vec<OscillatorPreset>,
    pub sweep: SweepPreset,
    pub binaural: BinauralPreset,
    /// Multiple notch filters (replaces the old single `notch` field).
    pub notch_filters: Vec<NotchPreset>,
    /// General audio filter chain.
    #[serde(default)]
    pub filters: Vec<FilterPreset>,
    /// Amplitude modulation settings.
    #[serde(default)]
    pub amp_mod: Option<AmpModPreset>,
    /// Residual inhibition settings.
    #[serde(default)]
    pub residual_inhibition: Option<ResidualInhibitionPreset>,
    /// Fractal tone settings.
    #[serde(default)]
    pub fractal_tones: Option<FractalTonePreset>,
    pub master_volume: f32,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            oscillators: vec![],
            sweep: SweepPreset {
                start_freq: 200.0,
                end_freq: 8000.0,
                duration_secs: 10.0,
                mode: SweepMode::Linear,
                volume: 0.3,
                enabled: false,
                looping: true,
            },
            binaural: BinauralPreset {
                base_freq: 440.0,
                beat_freq: 10.0,
                volume: 0.3,
                enabled: false,
            },
            notch_filters: vec![],
            filters: vec![],
            amp_mod: None,
            residual_inhibition: None,
            fractal_tones: None,
            master_volume: 0.5,
        }
    }
}

/// Get the directory where presets are stored.
pub fn presets_dir() -> PathBuf {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("tinnitus_freq_generator").join("presets");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Save a preset to disk.
pub fn save_preset(preset: &Preset) -> Result<PathBuf, String> {
    let dir = presets_dir();
    let filename = sanitize_filename(&preset.name);
    let path = dir.join(format!("{}.json", filename));
    let json = serde_json::to_string_pretty(preset)
        .map_err(|e| format!("Failed to serialize preset: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write preset file: {}", e))?;
    Ok(path)
}

/// Load a preset from disk.
pub fn load_preset(path: &std::path::Path) -> Result<Preset, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read preset file: {}", e))?;
    serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse preset: {}", e))
}

/// List all saved presets.
pub fn list_presets() -> Vec<(String, PathBuf)> {
    let dir = presets_dir();
    let mut presets = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(preset) = load_preset(&path) {
                    presets.push((preset.name, path));
                }
            }
        }
    }

    presets.sort_by(|a, b| a.0.cmp(&b.0));
    presets
}

/// Delete a preset file.
pub fn delete_preset(path: &std::path::Path) -> Result<(), String> {
    std::fs::remove_file(path)
        .map_err(|e| format!("Failed to delete preset: {}", e))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Built-in starter presets for common tinnitus frequencies.
pub fn builtin_presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "High-Pitch Tinnitus Relief".to_string(),
            oscillators: vec![
                OscillatorPreset {
                    waveform: Waveform::Sine,
                    frequency: 6000.0,
                    volume: 0.15,
                    enabled: true,
                    pan: 0.0,
                },
                OscillatorPreset {
                    waveform: Waveform::PinkNoise,
                    frequency: 1000.0,
                    volume: 0.2,
                    enabled: true,
                    pan: 0.0,
                },
            ],
            notch_filters: vec![
                NotchPreset {
                    center_freq: 6000.0,
                    bandwidth: 800.0,
                    enabled: true,
                    depth: 0.0,
                },
            ],
            ..Preset::default()
        },
        Preset {
            name: "Dual Notch Therapy".to_string(),
            oscillators: vec![
                OscillatorPreset {
                    waveform: Waveform::PinkNoise,
                    frequency: 1000.0,
                    volume: 0.3,
                    enabled: true,
                    pan: 0.0,
                },
            ],
            notch_filters: vec![
                NotchPreset {
                    center_freq: 4000.0,
                    bandwidth: 500.0,
                    enabled: true,
                    depth: 0.0,
                },
                NotchPreset {
                    center_freq: 8000.0,
                    bandwidth: 600.0,
                    enabled: true,
                    depth: 0.0,
                },
            ],
            ..Preset::default()
        },
        Preset {
            name: "Calming Binaural Alpha".to_string(),
            oscillators: vec![],
            binaural: BinauralPreset {
                base_freq: 300.0,
                beat_freq: 10.0,
                volume: 0.25,
                enabled: true,
            },
            ..Preset::default()
        },
        Preset {
            name: "White Noise Masking".to_string(),
            oscillators: vec![
                OscillatorPreset {
                    waveform: Waveform::WhiteNoise,
                    frequency: 1000.0,
                    volume: 0.25,
                    enabled: true,
                    pan: 0.0,
                },
            ],
            ..Preset::default()
        },
        Preset {
            name: "Gentle Sweep".to_string(),
            oscillators: vec![],
            sweep: SweepPreset {
                start_freq: 250.0,
                end_freq: 4000.0,
                duration_secs: 15.0,
                mode: SweepMode::Logarithmic,
                volume: 0.2,
                enabled: true,
                looping: true,
            },
            ..Preset::default()
        },
    ]
}
