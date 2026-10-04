use crate::types::*;
use std::collections::{BTreeMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

pub fn to_key(price: f64) -> i64 {
    (price * 100.0).round() as i64
}

pub fn from_key(key: i64) -> f64 {
    (key as f64) / 100.0
}

#[derive(Debug, Clone)]
pub struct LadderRow {
    pub price: f64,
    pub size: f64,
    pub cumulative: f64,
    pub is_wall: bool,
}

pub struct MarketState {
    // Ticker
    pub last_price: f64,
    pub prev_price: f64,
    pub price_change_24h: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub turnover_24h: f64,
    pub volume_24h: f64,
    pub funding_rate: f64,
    pub next_funding_time: i64,
    pub mark_price: f64,
    pub index_price: f64,
    pub open_interest: f64,
    pub open_interest_val: f64,

    // Arbitrage
    pub binance_price: f64,

    // Orderbook (key: price * 100, val: size in BTC)
    pub bids: BTreeMap<i64, f64>,
    pub asks: BTreeMap<i64, f64>,

    // Trades
    pub pending_trades: Vec<(f64, f64, String)>,
    pub trade_ticks: VecDeque<TradeTick>,
    pub large_trades: VecDeque<LargeTrade>,
    pub buy_vol_session: f64,
    pub sell_vol_session: f64,

    // Klines
    pub klines: Vec<Kline>,

    // Heatmap slices (1s intervals)
    pub history_slices: VecDeque<HistorySlice>,
    pub last_slice_ts: f64,

    // Detections & Alerts
    pub latest_whale: Option<LargeTrade>,
    pub whale_ts: f64,

    pub latest_anomaly: Option<String>,
    pub anomaly_ts: f64,

    pub vacuum_status: String,
    pub vacuum_alert: Option<String>,
    pub vacuum_ts: f64,

    pub latest_spoof: Option<String>,
    pub spoof_ts: f64,

    pub latest_liq: Option<String>,
    pub liq_ts: f64,
    pub long_liq_usd: f64,
    pub short_liq_usd: f64,

    // Interactive & Settings
    pub current_page: usize,
    pub is_menu_open: bool,
    pub menu_idx: usize,
    pub settings: Settings,
    pub ws_updates: u64,
}

impl MarketState {
    pub fn new() -> Self {
        let settings = Settings::load();
        let default_page = settings.default_page;
        Self {
            last_price: 0.0,
            prev_price: 0.0,
            price_change_24h: 0.0,
            high_24h: 0.0,
            low_24h: 0.0,
            turnover_24h: 0.0,
            volume_24h: 0.0,
            funding_rate: 0.0,
            next_funding_time: 0,
            mark_price: 0.0,
            index_price: 0.0,
            open_interest: 0.0,
            open_interest_val: 0.0,

            binance_price: 0.0,

            bids: BTreeMap::new(),
            asks: BTreeMap::new(),

            pending_trades: Vec::new(),
            trade_ticks: VecDeque::with_capacity(500),
            large_trades: VecDeque::with_capacity(30),
            buy_vol_session: 0.0,
            sell_vol_session: 0.0,

            klines: Vec::new(),
            history_slices: VecDeque::with_capacity(70),
            last_slice_ts: 0.0,

            latest_whale: None,
            whale_ts: 0.0,

            latest_anomaly: None,
            anomaly_ts: 0.0,

            vacuum_status: "BALANCED".to_string(),
            vacuum_alert: None,
            vacuum_ts: 0.0,

            latest_spoof: None,
            spoof_ts: 0.0,

            latest_liq: None,
            liq_ts: 0.0,
            long_liq_usd: 0.0,
            short_liq_usd: 0.0,

            current_page: default_page,
            is_menu_open: false,
            menu_idx: 0,
            settings,
            ws_updates: 0,
        }
    }

    pub fn update_ticker(&mut self, data: &serde_json::Value) {
        if let Some(p) = data.get("lastPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            if self.last_price > 0.0 {
                self.prev_price = self.last_price;
            }
            self.last_price = p;
        }
        if let Some(p) = data.get("price24hPcnt").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()) {
            self.price_change_24h = p * 100.0;
        }
        if let Some(p) = data.get("highPrice24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.high_24h = p;
        }
        if let Some(p) = data.get("lowPrice24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.low_24h = p;
        }
        if let Some(p) = data.get("turnover24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.turnover_24h = p;
        }
        if let Some(p) = data.get("volume24h").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.volume_24h = p;
        }
        if let Some(p) = data.get("fundingRate").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()) {
            self.funding_rate = p * 100.0;
        }
        if let Some(p) = data.get("nextFundingTime").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.next_funding_time = p;
        }
        if let Some(p) = data.get("markPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.mark_price = p;
        }
        if let Some(p) = data.get("indexPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.index_price = p;
        }
        if let Some(p) = data.get("openInterest").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.open_interest = p;
        }
        if let Some(p) = data.get("openInterestValue").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
            self.open_interest_val = p;
        }
        self.ws_updates += 1;
    }

    pub fn update_orderbook(&mut self, msg_type: &str, data: &serde_json::Value) {
        let now = now_secs();
        if msg_type == "snapshot" {
            self.bids.clear();
            self.asks.clear();
            if let Some(bids) = data.get("b").and_then(|v| v.as_array()) {
                for item in bids {
                    if let (Some(p), Some(s)) = (
                        item.get(0).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                        item.get(1).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                    ) {
                        if s > 0.0 {
                            self.bids.insert(to_key(p), s);
                        }
                    }
                }
            }
            if let Some(asks) = data.get("a").and_then(|v| v.as_array()) {
                for item in asks {
                    if let (Some(p), Some(s)) = (
                        item.get(0).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                        item.get(1).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                    ) {
                        if s > 0.0 {
                            self.asks.insert(to_key(p), s);
                        }
                    }
                }
            }
        } else if msg_type == "delta" {
            if let Some(bids) = data.get("b").and_then(|v| v.as_array()) {
                for item in bids {
                    if let (Some(p), Some(s)) = (
                        item.get(0).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                        item.get(1).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                    ) {
                        let k = to_key(p);
                        let prev_s = self.bids.get(&k).copied().unwrap_or(s);
                        let diff = prev_s - s;
                        if diff >= 2.5 {
                            self.latest_spoof = Some(format!("BID PULL: -{:.1} BTC @ ${:.0}", diff, p));
                            self.spoof_ts = now;
                        }
                        if s <= 0.0 {
                            self.bids.remove(&k);
                        } else {
                            self.bids.insert(k, s);
                        }
                    }
                }
            }
            if let Some(asks) = data.get("a").and_then(|v| v.as_array()) {
                for item in asks {
                    if let (Some(p), Some(s)) = (
                        item.get(0).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                        item.get(1).and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()),
                    ) {
                        let k = to_key(p);
                        let prev_s = self.asks.get(&k).copied().unwrap_or(s);
                        let diff = prev_s - s;
                        if diff >= 2.5 {
                            self.latest_spoof = Some(format!("ASK PULL: -{:.1} BTC @ ${:.0}", diff, p));
                            self.spoof_ts = now;
                        }
                        if s <= 0.0 {
                            self.asks.remove(&k);
                        } else {
                            self.asks.insert(k, s);
                        }
                    }
                }
            }
        }
        self.ws_updates += 1;
    }

    pub fn update_trades(&mut self, trade_list: &[serde_json::Value]) {
        let now = now_secs();
        let thresh = self.settings.whale_threshold;

        for tr in trade_list {
            let side = tr.get("S").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let vol = tr.get("v").and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
            let price = tr.get("p").and_then(|v| v.as_str()).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);

            if side == "Buy" {
                self.buy_vol_session += vol;
            } else if side == "Sell" {
                self.sell_vol_session += vol;
            }

            if price > 0.0 && vol > 0.0 && (side == "Buy" || side == "Sell") {
                self.pending_trades.push((price, vol, side.clone()));
                self.trade_ticks.push_back(TradeTick {
                    ts: now,
                    vol,
                    side: side.clone(),
                    price,
                });
                if self.trade_ticks.len() > 400 {
                    self.trade_ticks.pop_front();
                }

                if vol >= thresh {
                    let w = LargeTrade {
                        ts: now,
                        side: side.clone(),
                        vol,
                        price,
                        usd: vol * price,
                    };
                    self.large_trades.push_front(w.clone());
                    if self.large_trades.len() > 20 {
                        self.large_trades.pop_back();
                    }
                    self.latest_whale = Some(w);
                    self.whale_ts = now;
                }
            }
        }
        self.ws_updates += 1;
    }

    pub fn update_kline(&mut self, kline_item: &serde_json::Value) {
        let start = kline_item.get("start").and_then(|v| v.as_i64()).unwrap_or(0);
        let open = kline_item.get("open").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let high = kline_item.get("high").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let low = kline_item.get("low").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let close = kline_item.get("close").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let volume = kline_item.get("volume").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let turnover = kline_item.get("turnover").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);

        let k = Kline {
            start,
            open,
            high,
            low,
            close,
            volume,
            turnover,
        };

        if let Some(first) = self.klines.first_mut() {
            if first.start == start {
                *first = k;
                self.ws_updates += 1;
                return;
            }
        }
        self.klines.insert(0, k);
        if self.klines.len() > 60 {
            self.klines.pop();
        }
        self.ws_updates += 1;
    }

    pub fn update_liquidation(&mut self, data: &serde_json::Value) {
        let side = data.get("side").and_then(|v| v.as_str()).unwrap_or("");
        let size = data.get("size").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        let price = data.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        let val_usd = size * price;

        let tag = if side == "Buy" {
            self.short_liq_usd += val_usd;
            "SHORT REKT"
        } else {
            self.long_liq_usd += val_usd;
            "LONG REKT"
        };

        self.latest_liq = Some(format!("{}: {:.2} BTC (${:.0}K) @ ${:.0}", tag, size, val_usd / 1000.0, price));
        self.liq_ts = now_secs();
        self.ws_updates += 1;
    }

    pub fn evaluate_anomalies_and_vacuum(&mut self) {
        let now = now_secs();
        if self.last_price == 0.0 || self.bids.is_empty() || self.asks.is_empty() {
            return;
        }

        let best_bid = from_key(*self.bids.keys().next_back().unwrap());
        let best_ask = from_key(*self.asks.keys().next().unwrap());
        let spread = (best_ask - best_bid).max(0.0);

        // 1. Evaluate Vacuum in +/- $25
        let ask_25: f64 = self.asks.iter()
            .filter(|(k, _)| from_key(**k) <= best_ask + 25.0)
            .map(|(_, s)| *s)
            .sum();
        let bid_25: f64 = self.bids.iter()
            .filter(|(k, _)| from_key(**k) >= best_bid - 25.0)
            .map(|(_, s)| *s)
            .sum();

        let ask_wall = self.asks.iter()
            .find(|(_, s)| **s >= 10.0)
            .map(|(k, s)| (from_key(*k), *s));
        let bid_wall = self.bids.iter().rev()
            .find(|(_, s)| **s >= 10.0)
            .map(|(k, s)| (from_key(*k), *s));

        if ask_25 < 1.8 {
            let wall_str = if let Some((p, s)) = ask_wall {
                format!("+${:.1} ({:.1}B)", p - best_ask, s)
            } else {
                "No Wall".to_string()
            };
            self.vacuum_status = format!("THIN ASK ({:.1}B in +$25)", ask_25);
            self.vacuum_alert = Some(format!("🔴 [VACUUM] THIN ASK ({:.1}B) │ Wall: {}", ask_25, wall_str));
            self.vacuum_ts = now;
        } else if bid_25 < 1.8 {
            let wall_str = if let Some((p, s)) = bid_wall {
                format!("-${:.1} ({:.1}B)", best_bid - p, s)
            } else {
                "No Wall".to_string()
            };
            self.vacuum_status = format!("THIN BID ({:.1}B in -$25)", bid_25);
            self.vacuum_alert = Some(format!("🔴 [VACUUM] THIN BID ({:.1}B) │ Wall: {}", bid_25, wall_str));
            self.vacuum_ts = now;
        } else {
            self.vacuum_status = format!("BALANCED ({:.1}B Bid / {:.1}B Ask in ±$25)", bid_25, ask_25);
        }

        // 2. Microstructure Anomalies
        let v_5s: f64 = self.trade_ticks.iter()
            .filter(|t| now - t.ts <= 5.0)
            .map(|t| t.vol)
            .sum();
        let v_30s: f64 = self.trade_ticks.iter()
            .filter(|t| now - t.ts <= 30.0)
            .map(|t| t.vol)
            .sum();
        let baseline_5s = if v_30s > 0.0 { v_30s / 6.0 } else { 0.5 };

        if v_5s >= 3.5 * baseline_5s && v_5s >= 5.0 {
            let ratio = v_5s / baseline_5s;
            self.latest_anomaly = Some(format!("🟠 [ANOMALY] VOL SPIKE: {:.1}x ({:.1} BTC/5s)", ratio, v_5s));
            self.anomaly_ts = now;
        } else if spread >= 0.40 {
            self.latest_anomaly = Some(format!("🟠 [ANOMALY] SPREAD WIDE: ${:.2} ({:.1}x)", spread, spread / 0.10));
            self.anomaly_ts = now;
        } else {
            let buy_5s: f64 = self.trade_ticks.iter()
                .filter(|t| now - t.ts <= 5.0 && t.side == "Buy")
                .map(|t| t.vol)
                .sum();
            let sell_5s: f64 = self.trade_ticks.iter()
                .filter(|t| now - t.ts <= 5.0 && t.side == "Sell")
                .map(|t| t.vol)
                .sum();
            let delta_5s = buy_5s - sell_5s;
            if delta_5s.abs() >= 8.0 {
                let sign = if delta_5s > 0.0 { "+" } else { "" };
                self.latest_anomaly = Some(format!("🟠 [ANOMALY] CVD SURGE: {}{:.1} BTC/5s", sign, delta_5s));
                self.anomaly_ts = now;
            }
        }
    }

    pub fn record_history_slice(&mut self) {
        if self.last_price == 0.0 {
            return;
        }
        let now = now_secs();
        let bids: Vec<(f64, f64)> = self.bids.iter().rev().take(30).map(|(k, s)| (from_key(*k), *s)).collect();
        let asks: Vec<(f64, f64)> = self.asks.iter().take(30).map(|(k, s)| (from_key(*k), *s)).collect();
        let trades = std::mem::take(&mut self.pending_trades);

        self.history_slices.push_back(HistorySlice {
            ts: now,
            price: self.last_price,
            bids,
            asks,
            trades,
        });

        if self.history_slices.len() > 60 {
            self.history_slices.pop_front();
        }
        self.last_slice_ts = now;
    }

    pub fn seed_initial_slices(&mut self, count: usize) {
        if !self.history_slices.is_empty() || self.last_price == 0.0 {
            return;
        }
        let now = now_secs();
        let c_open = self.klines.first().map(|k| k.open).unwrap_or(self.last_price);
        let c_high = self.klines.first().map(|k| k.high).unwrap_or(self.last_price);
        let c_low = self.klines.first().map(|k| k.low).unwrap_or(self.last_price);

        let bids: Vec<(f64, f64)> = self.bids.iter().rev().take(30).map(|(k, s)| (from_key(*k), *s)).collect();
        let asks: Vec<(f64, f64)> = self.asks.iter().take(30).map(|(k, s)| (from_key(*k), *s)).collect();

        for i in 0..count {
            let t = (i as f64) / ((count.max(2) - 1) as f64);
            let p = (c_open + (self.last_price - c_open) * t).clamp(c_low, c_high);
            let ts = now - ((count - i) as f64);
            let mut trades = Vec::new();
            if i == count / 4 || i == count / 2 || i == (3 * count) / 4 {
                let side = if p >= c_open { "Buy" } else { "Sell" };
                trades.push((p, 0.4, side.to_string()));
            }
            self.history_slices.push_back(HistorySlice {
                ts,
                price: p,
                bids: bids.clone(),
                asks: asks.clone(),
                trades,
            });
        }
    }

    pub fn calculate_rsi(&self, period: usize) -> f64 {
        if self.klines.len() < period + 1 {
            return 50.0;
        }
        let mut closes: Vec<f64> = self.klines.iter().rev().map(|k| k.close).collect();
        if closes.len() > period + 14 {
            closes = closes[closes.len() - (period + 14)..].to_vec();
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in 1..=period {
            let diff = closes[i] - closes[i - 1];
            if diff >= 0.0 {
                gains += diff;
            } else {
                losses += -diff;
            }
        }

        let mut avg_gain = gains / (period as f64);
        let mut avg_loss = losses / (period as f64);

        for i in (period + 1)..closes.len() {
            let diff = closes[i] - closes[i - 1];
            let gain = if diff > 0.0 { diff } else { 0.0 };
            let loss = if diff < 0.0 { -diff } else { 0.0 };
            avg_gain = (avg_gain * ((period - 1) as f64) + gain) / (period as f64);
            avg_loss = (avg_loss * ((period - 1) as f64) + loss) / (period as f64);
        }

        if avg_loss == 0.0 {
            return 100.0;
        }
        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    pub fn calculate_ema(&self, period: usize) -> f64 {
        if self.klines.is_empty() {
            return self.last_price;
        }
        let closes: Vec<f64> = self.klines.iter().rev().map(|k| k.close).collect();
        if closes.is_empty() {
            return self.last_price;
        }
        let k = 2.0 / ((period + 1) as f64);
        let mut ema = closes[0];
        for price in closes.iter().skip(1) {
            ema = price * k + ema * (1.0 - k);
        }
        ema
    }

    pub fn get_orderbook_pressure(&self, levels: usize) -> (f64, f64, f64) {
        let bid_vol: f64 = self.bids.iter().rev().take(levels).map(|(_, s)| *s).sum();
        let ask_vol: f64 = self.asks.iter().take(levels).map(|(_, s)| *s).sum();
        let total = bid_vol + ask_vol;
        let bid_pct = if total > 0.0 { (bid_vol / total) * 100.0 } else { 50.0 };
        (bid_pct, bid_vol, ask_vol)
    }

    pub fn evaluate_confluence(&self) -> (i32, String, String) {
        let rsi = self.calculate_rsi(14);
        let ema9 = self.calculate_ema(9);
        let ema21 = self.calculate_ema(21);
        let (bid_ratio, _, _) = self.get_orderbook_pressure(20);
        let basis = self.last_price - self.index_price;
        let range_diff = self.high_24h - self.low_24h;
        let range_pct = if range_diff > 0.0 { (self.last_price - self.low_24h) / range_diff } else { 0.5 };

        let mut score: i32 = 0;

        // 1. Technical
        if rsi < 30.0 { score += 15; }
        else if rsi > 70.0 { score -= 15; }

        if ema9 > ema21 { score += 15; }
        else if ema9 < ema21 { score -= 15; }

        // 2. Microstructure
        if bid_ratio > 55.0 { score += 20; }
        else if bid_ratio < 45.0 { score -= 20; }

        // 3. Derivatives
        if basis > 0.0 { score += 10; }
        else if basis < 0.0 { score -= 10; }

        if self.funding_rate < 0.0 { score += 10; }
        else if self.funding_rate > 0.01 { score -= 10; }

        // 4. Mean Reversion
        if range_pct > 0.8 { score -= 10; }
        else if range_pct < 0.2 { score += 10; }

        score = score.clamp(-100, 100);

        let bias = if score >= 45 {
            "STRONG BULLISH ▲".to_string()
        } else if score >= 15 {
            "LEAN BUY ▲".to_string()
        } else if score <= -45 {
            "STRONG BEARISH ▼".to_string()
        } else if score <= -15 {
            "LEAN SELL ▼".to_string()
        } else {
            "NEUTRAL / RANGE ─".to_string()
        };

        let setup = if self.funding_rate < -0.002 && rsi < 42.0 && bid_ratio > 55.0 {
            "🟡 [SQUEEZE] Short Squeeze Candidate".to_string()
        } else if self.funding_rate > 0.015 && rsi > 68.0 && bid_ratio < 45.0 {
            "🟠 [FLUSH] Long Flush / Exhaustion".to_string()
        } else if ema9 > ema21 && bid_ratio > 60.0 && basis > 0.0 {
            "🟢 [MOMENTUM] Bullish Momentum Inflow".to_string()
        } else if ema9 < ema21 && bid_ratio < 40.0 && basis < 0.0 {
            "🔴 [DISTRIBUTION] Bearish Distribution Flow".to_string()
        } else if (40.0..=60.0).contains(&rsi) && score.abs() < 20 {
            "● [RANGE] Mean-Reversion / Chop".to_string()
        } else {
            "● [NEUTRAL] Confluence Building...".to_string()
        };

        (score, bias, setup)
    }

    pub fn get_ladder_rows(&self, step: f64, levels: usize) -> (Vec<LadderRow>, Vec<LadderRow>, (f64, f64), (f64, f64)) {
        if self.bids.is_empty() || self.asks.is_empty() {
            return (Vec::new(), Vec::new(), (0.0, 0.0), (0.0, 0.0));
        }

        let best_bid = from_key(*self.bids.keys().next_back().unwrap());
        let best_ask = from_key(*self.asks.keys().next().unwrap());

        let mut ask_buckets: BTreeMap<i64, f64> = BTreeMap::new();
        for (k, s) in &self.asks {
            let p = from_key(*k);
            let bucket = ((p / step).ceil() * step * 100.0).round() as i64;
            *ask_buckets.entry(bucket).or_insert(0.0) += s;
        }

        let mut bid_buckets: BTreeMap<i64, f64> = BTreeMap::new();
        for (k, s) in &self.bids {
            let p = from_key(*k);
            let bucket = ((p / step).floor() * step * 100.0).round() as i64;
            *bid_buckets.entry(bucket).or_insert(0.0) += s;
        }

        // Asks descending down to mid price
        let mut ask_rows: Vec<LadderRow> = Vec::new();
        let mut ask_cumul = 0.0;
        let mut ask_wall = (0.0, 0.0);

        for (k, s) in ask_buckets.iter().take(levels) {
            let p = from_key(*k);
            ask_cumul += s;
            let is_wall = *s >= 8.0;
            if *s > ask_wall.1 {
                ask_wall = (p, *s);
            }
            ask_rows.push(LadderRow {
                price: p,
                size: *s,
                cumulative: ask_cumul,
                is_wall,
            });
        }
        // Reverse so highest ask is at top, lowest ask near mid
        ask_rows.reverse();

        // Bids descending from mid price down
        let mut bid_rows: Vec<LadderRow> = Vec::new();
        let mut bid_cumul = 0.0;
        let mut bid_wall = (0.0, 0.0);

        for (k, s) in bid_buckets.iter().rev().take(levels) {
            let p = from_key(*k);
            bid_cumul += s;
            let is_wall = *s >= 8.0;
            if *s > bid_wall.1 {
                bid_wall = (p, *s);
            }
            bid_rows.push(LadderRow {
                price: p,
                size: *s,
                cumulative: bid_cumul,
                is_wall,
            });
        }

        if ask_wall.0 == 0.0 {
            ask_wall = (best_ask, 0.0);
        }
        if bid_wall.0 == 0.0 {
            bid_wall = (best_bid, 0.0);
        }

        (ask_rows, bid_rows, bid_wall, ask_wall)
    }
}
