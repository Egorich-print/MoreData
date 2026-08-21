//! Plugin abstraction. Night-1: native DSP nodes only. Hosts are adapters.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginFormat {
    Native,
    Clap,
    Lv2,
    Vst3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RealtimeSafety {
    Safe,
    Unknown,
    Unsafe,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub format: PluginFormat,
    pub realtime: RealtimeSafety,
    pub headless_safe: bool,
    pub gui_required: bool,
    pub sandboxable: bool,
    pub stateful: bool,
    pub latency_frames: u32,
    pub tail_frames: u32,
}

impl PluginInfo {
    pub fn native(id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            format: PluginFormat::Native,
            realtime: RealtimeSafety::Safe,
            headless_safe: true,
            gui_required: false,
            sandboxable: true,
            stateful: true,
            latency_frames: 0,
            tail_frames: 0,
        }
    }

    pub fn compatible(&self) -> bool {
        self.headless_safe && !self.gui_required && self.realtime != RealtimeSafety::Unsafe
    }
}

pub fn builtin_catalog() -> Vec<PluginInfo> {
    vec![
        PluginInfo::native("oscillator", "Oscillator"),
        PluginInfo::native("gain", "Gain"),
        PluginInfo::native("mixer", "Mixer"),
        PluginInfo::native("output", "Output"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natives_are_headless() {
        for p in builtin_catalog() {
            assert!(p.compatible());
            assert_eq!(p.format, PluginFormat::Native);
        }
    }
}
