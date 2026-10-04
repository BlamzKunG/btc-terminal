use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_ladder_rows() -> usize {
    10
}

fn default_enable_web_ui() -> bool {
    true
}

fn default_web_port() -> u16 {
    8080
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_page: usize,
    pub show_signal: bool,
    pub show_technical: bool,
    pub show_whale_radar: bool,
    pub show_vacuum_radar: bool,
    pub show_microstructure: bool,
    pub show_derivatives: bool,
    pub show_candles: bool,
    pub ladder_step: f64,
    pub whale_threshold: f64,
    #[serde(default = "default_ladder_rows")]
    pub ladder_rows: usize,
    #[serde(default = "default_enable_web_ui")]
    pub enable_web_ui: bool,
    #[serde(default = "default_web_port")]
    pub web_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_page: 1,
            show_signal: true,
            show_technical: true,
            show_whale_radar: true,
            show_vacuum_radar: true,
            show_microstructure: true,
            show_derivatives: true,
            show_candles: true,
            ladder_step: 1.0,
            whale_threshold: 2.0,
            ladder_rows: 10,
            enable_web_ui: true,
            web_port: 8080,
        }
    }
}

impl Settings {
    fn config_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config/btc/settings.json")
        } else {
            PathBuf::from("/root/.config/btc/settings.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, data);
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeTick {
    pub ts: f64,
    pub vol: f64,
    pub side: String,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeTrade {
    pub ts: f64,
    pub side: String,
    pub vol: f64,
    pub price: f64,
    pub usd: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    pub start: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub turnover: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySlice {
    pub ts: f64,
    pub price: f64,
    pub bids: Vec<(f64, f64)>, // (price, size)
    pub asks: Vec<(f64, f64)>,
    pub trades: Vec<(f64, f64, String)>, // (price, vol, side)
}
