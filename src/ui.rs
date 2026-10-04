use crate::state::{from_key, MarketState};
use chrono::{Local, TimeZone};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Widget},
};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

pub fn render_spans(buf: &mut Buffer, x: u16, y: u16, spans: &[(&str, Style)]) {
    if y >= buf.area.height {
        return;
    }
    let mut cur_x = x;
    for (text, style) in spans {
        if cur_x >= buf.area.width {
            break;
        }
        buf.set_string(cur_x, y, *text, *style);
        cur_x += text.width() as u16;
    }
}

pub fn format_comma(num: f64, decimals: usize) -> String {
    let sign = if num < 0.0 { "-" } else { "" };
    let abs_n = num.abs();
    let int_part = abs_n.trunc() as u64;
    let s = int_part.to_string();
    let mut with_commas = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(c);
        count += 1;
    }
    let reversed: String = with_commas.chars().rev().collect();
    if decimals > 0 {
        let frac_part = ((abs_n.fract() * 10f64.powi(decimals as i32)).round() as u64) % 10u64.pow(decimals as u32);
        format!("{}{}.{:0width$}", sign, reversed, frac_part, width = decimals)
    } else {
        format!("{}{}", sign, reversed)
    }
}

pub fn format_usd(val: f64) -> String {
    if val >= 1_000_000_000.0 {
        format!("${:.2}B", val / 1_000_000_000.0)
    } else if val >= 1_000_000.0 {
        format!("${:.2}M", val / 1_000_000.0)
    } else if val >= 1_000.0 {
        format!("${:.0}K", val / 1_000.0)
    } else {
        format!("${:.2}", val)
    }
}

pub fn format_vol(val: f64) -> String {
    if val >= 1_000_000.0 {
        format!("{:.1}M", val / 1_000_000.0)
    } else if val >= 1_000.0 {
        format!("{:.0}K", val / 1_000.0)
    } else {
        format!("{:.2}", val)
    }
}

pub fn render_pressure_bar(ratio: f64, length: usize) -> (String, String) {
    let r = ratio.clamp(0.0, 100.0) / 100.0;
    let b_len = (r * (length as f64)).round() as usize;
    let a_len = length.saturating_sub(b_len);
    ("█".repeat(b_len), "█".repeat(a_len))
}

pub fn render_score_gauge(score: i32, length: usize) -> String {
    let half = (length - 1) / 2;
    let normalized = (score.clamp(-100, 100) as f64) / 100.0;
    let dot_pos = ((normalized + 1.0) / 2.0 * ((length - 1) as f64)).round() as usize;
    let mut chars: Vec<char> = vec!['─'; length];
    if half < length {
        chars[half] = '│';
    }
    if dot_pos < length {
        chars[dot_pos] = '●';
    }
    chars.into_iter().collect()
}

pub fn render_range_bar(val: f64, min_v: f64, max_v: f64, length: usize) -> String {
    let diff = max_v - min_v;
    if diff <= 0.0 {
        return "━".repeat(length);
    }
    let pct = ((val - min_v) / diff).clamp(0.0, 1.0);
    let dot_idx = (pct * ((length - 1) as f64)).round() as usize;
    let mut chars: Vec<char> = vec!['━'; length];
    if dot_idx < length {
        chars[dot_idx] = '●';
    }
    chars.into_iter().collect()
}

const SPARK_BARS: &[char] = &[' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
pub fn generate_sparkline(prices: &[f64]) -> String {
    if prices.len() < 2 {
        return "".to_string();
    }
    let min_p = prices.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_p = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let diff = max_p - min_p;
    if diff == 0.0 {
        return "▄".repeat(prices.len());
    }
    let mut s = String::with_capacity(prices.len());
    for p in prices {
        let idx = (((p - min_p) / diff) * ((SPARK_BARS.len() - 1) as f64)).round() as usize;
        s.push(SPARK_BARS[idx.clamp(0, SPARK_BARS.len() - 1)]);
    }
    s
}

pub struct TerminalView<'a> {
    pub state: &'a MarketState,
}

impl<'a> TerminalView<'a> {
    pub fn new(state: &'a MarketState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for TerminalView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let st = self.state;

        // 1. Horizontal Fit: Center the 74-character box on wide terminals
        let block_w = 74u16.min(area.width);
        let block_x = area.x + (area.width.saturating_sub(block_w)) / 2;

        let has_footer = area.height >= 12;
        let max_allowed_h = if has_footer { area.height.saturating_sub(1) } else { area.height };

        // 2. Vertical Fit: Dynamically fit height based on active page content
        let needed_h = match st.current_page {
            1 => {
                let mut rows = 2; // tabs + top divider
                rows += 4; // hero, 24h, sparkline, arb
                if st.settings.show_signal { rows += 4; }
                if st.settings.show_technical { rows += 4; }
                if st.settings.show_whale_radar {
                    let w_count = st.large_trades.len().clamp(1, 3) as u16;
                    rows += 2 + w_count;
                }
                if st.settings.show_vacuum_radar { rows += 4; }
                if st.settings.show_microstructure { rows += 6; }
                if st.settings.show_derivatives { rows += 5; }
                if st.settings.show_candles {
                    let candle_count = ((max_allowed_h.saturating_sub(rows + 5)) as usize).clamp(3, 5) as u16;
                    rows += 3 + candle_count;
                }
                rows + 2 // borders
            }
            2 => {
                let avail_space = max_allowed_h.saturating_sub(13) as usize;
                let max_depth = avail_space / 2;
                let depth = st.settings.ladder_rows.min(max_depth).max(3);
                15 + (depth as u16) * 2
            }
            _ => {
                let avail_space = max_allowed_h.saturating_sub(14) as usize;
                let tiers = (st.settings.ladder_rows * 2).min(avail_space).max(6);
                14 + (tiers as u16)
            }
        };

        let block_h = needed_h.min(max_allowed_h);
        let block_y = area.y;

        let render_area = Rect {
            x: block_x,
            y: block_y,
            width: block_w,
            height: block_h,
        };

        // Theme border color per page
        let border_color = match st.current_page {
            1 => Color::Cyan,
            2 => Color::Rgb(255, 200, 50),
            _ => Color::Rgb(215, 100, 255),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(render_area);
        block.render(render_area, buf);

        if has_footer && render_area.bottom() < area.bottom() + 1 {
            let step_str = format!(" Step: ${:.0} │ ", st.settings.ladder_step);
            let depth_str = format!(" Depth: {} │ ", st.settings.ladder_rows);
            let whale_str = format!(" Whale: ≥{:.1}B │ ", st.settings.whale_threshold);
            let footer_spans: &[(&str, Style)] = &[
                ("[Tab]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (" Page │ ", Style::default().fg(Color::Rgb(160, 175, 195))),
                ("[+/-]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (&step_str, Style::default().fg(Color::Rgb(160, 175, 195))),
                ("[r]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (&depth_str, Style::default().fg(Color::Rgb(160, 175, 195))),
                ("[w]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (&whale_str, Style::default().fg(Color::Rgb(160, 175, 195))),
                ("[m]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (" Menu │ ", Style::default().fg(Color::Rgb(160, 175, 195))),
                ("[q]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (" Exit", Style::default().fg(Color::Rgb(160, 175, 195))),
            ];
            render_spans(buf, render_area.x + 1, render_area.bottom(), footer_spans);
        }

        if inner.width < 60 || inner.height < 10 {
            return;
        }

        let inner_w = inner.width as usize;
        let mut row = inner.y;

        // Top Tab Bar with High-Contrast Pill Badges
        render_top_tabs(st.current_page, inner_w, inner.x, row, buf);
        row += 1;

        draw_horizontal_divider(inner.x, row, inner_w, border_color, buf);
        row += 1;

        if st.is_menu_open {
            render_menu(st, inner_w, inner.x, row, buf);
            return;
        }

        match st.current_page {
            1 => render_dashboard_page(st, inner_w, inner.x, &mut row, inner.bottom(), border_color, buf),
            2 => render_ladder_page(st, inner_w, inner.x, &mut row, inner.bottom(), border_color, buf),
            _ => render_heatmap_page(st, inner_w, inner.x, &mut row, inner.bottom(), border_color, buf),
        }
    }
}

fn draw_horizontal_divider(x: u16, y: u16, width: usize, color: Color, buf: &mut Buffer) {
    if y >= buf.area.height { return; }
    let s = "═".repeat(width);
    buf.set_string(x, y, &s, Style::default().fg(color));
    if x > 0 && y < buf.area.height {
        if let Some(cell) = buf.cell_mut((x - 1, y)) {
            cell.set_symbol("╠").set_style(Style::default().fg(color));
        }
    }
    let rx = x + width as u16;
    if rx < buf.area.width && y < buf.area.height {
        if let Some(cell) = buf.cell_mut((rx, y)) {
            cell.set_symbol("╣").set_style(Style::default().fg(color));
        }
    }
}

fn render_top_tabs(current_page: usize, inner_w: usize, x: u16, y: u16, buf: &mut Buffer) {
    let mut cur_x = x + 1;

    // Tab 1: DASHBOARD
    if current_page == 1 {
        let t1 = " 1 DASHBOARD ";
        buf.set_string(cur_x, y, t1, Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD));
        cur_x += t1.chars().count() as u16 + 1;
    } else {
        buf.set_string(cur_x, y, "[1]", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        buf.set_string(cur_x + 3, y, " DASHBOARD ", Style::default().fg(Color::Rgb(160, 175, 195)));
        cur_x += 15;
    }

    // Tab 2: LADDER
    if current_page == 2 {
        let t2 = " 2 LADDER ";
        buf.set_string(cur_x, y, t2, Style::default().bg(Color::Rgb(255, 200, 50)).fg(Color::Black).add_modifier(Modifier::BOLD));
        cur_x += t2.chars().count() as u16 + 1;
    } else {
        buf.set_string(cur_x, y, "[2]", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        buf.set_string(cur_x + 3, y, " LADDER ", Style::default().fg(Color::Rgb(160, 175, 195)));
        cur_x += 12;
    }

    // Tab 3: 2D HEATMAP
    if current_page == 3 {
        let t3 = " 3 2D HEATMAP ";
        buf.set_string(cur_x, y, t3, Style::default().bg(Color::Rgb(215, 100, 255)).fg(Color::Black).add_modifier(Modifier::BOLD));
    } else {
        buf.set_string(cur_x, y, "[3]", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        buf.set_string(cur_x + 3, y, " 2D HEATMAP", Style::default().fg(Color::Rgb(160, 175, 195)));
    }

    // Switch hint at right
    let hint_spans: &[(&str, Style)] = &[
        ("[Tab]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        (" Next", Style::default().fg(Color::Rgb(160, 175, 195))),
    ];
    let hint_len = 10;
    let hint_x = (x + inner_w as u16).saturating_sub(hint_len);
    render_spans(buf, hint_x, y, hint_spans);
}

fn render_menu(st: &MarketState, inner_w: usize, x: u16, start_y: u16, buf: &mut Buffer) {
    let mut row = start_y;
    buf.set_string(x + 2, row, "[SETTINGS] TERMINAL DISPLAY CONFIGURATION", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    row += 1;
    buf.set_string(x + 2, row, "Use [↑/↓] Navigate  •  [Enter] Toggle  •  [m/Esc] Done", Style::default().fg(Color::Rgb(160, 175, 195)));
    row += 1;

    draw_horizontal_divider(x, row, inner_w, Color::Yellow, buf);
    row += 2;

    let items = [
        ("4-Layer Confluence Signal Engine", st.settings.show_signal),
        ("Technical Signals (RSI / EMA)", st.settings.show_technical),
        ("Whale / Large Trade Radar", st.settings.show_whale_radar),
        ("Liquidity Vacuum & Anomaly Detector", st.settings.show_vacuum_radar),
        ("Orderbook Microstructure & Pressure", st.settings.show_microstructure),
        ("Derivatives & Market Metrics", st.settings.show_derivatives),
        ("Latest 1-Min Candlesticks", st.settings.show_candles),
    ];

    for (idx, (label, is_on)) in items.iter().enumerate() {
        let is_cur = idx == st.menu_idx;
        let pointer = if is_cur { "► " } else { "  " };
        let check = if *is_on { "[●] ON " } else { "[○] OFF" };
        let check_col = if *is_on { Color::Green } else { Color::DarkGray };
        let text_style = if is_cur {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if *is_on {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        buf.set_string(x + 2, row, pointer, Style::default().fg(Color::Yellow));
        buf.set_string(x + 4, row, check, Style::default().fg(check_col));
        buf.set_string(x + 13, row, *label, text_style);
        row += 1;
    }

    row += 1;
    buf.set_string(x + 2, row, &format!("BookMap Aggregation Step: ${:.1} (Change with [+] and [-])", st.settings.ladder_step), Style::default().fg(Color::Rgb(160, 175, 195)));
    row += 1;
    buf.set_string(x + 2, row, &format!("BookMap / Ladder Depth  : {} Rows (Change with [r] or [ / ])", st.settings.ladder_rows), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    row += 1;
    buf.set_string(x + 2, row, &format!("Whale Radar Threshold   : ≥{:.1} BTC (Change with [w])", st.settings.whale_threshold), Style::default().fg(Color::Rgb(160, 175, 195)));
    row += 2;

    draw_horizontal_divider(x, row, inner_w, Color::Yellow, buf);
    row += 1;
    buf.set_string(x + 2, row, "Tip: Settings auto-saved to ~/.config/btc/settings.json", Style::default().fg(Color::DarkGray));
    row += 1;
    buf.set_string(x + 2, row, "Press [Enter] to Toggle  │  Press [m] or [Esc] to Return", Style::default().fg(Color::White));
}

fn render_dashboard_page(st: &MarketState, inner_w: usize, x: u16, row: &mut u16, max_y: u16, border_col: Color, buf: &mut Buffer) {
    if *row >= max_y { return; }

    let lbl_style = Style::default().fg(Color::Rgb(160, 175, 195));

    // 1. Hero row
    let sym = "BTCUSDT";
    let p_col = if st.last_price >= st.prev_price { Color::LightGreen } else { Color::LightRed };
    let tick_icon = if st.last_price >= st.prev_price { "▲" } else { "▼" };
    let chg_sign = if st.price_change_24h >= 0.0 { "+" } else { "" };
    let px_s = format_comma(st.last_price, 2);
    let chg_s = format!("{}{:.2}%", chg_sign, st.price_change_24h);
    let hero_spans: &[(&str, Style)] = &[
        ("  ", Style::default()),
        (sym, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ("  $", Style::default().fg(p_col)),
        (&px_s, Style::default().fg(p_col).add_modifier(Modifier::BOLD)),
        (" ", Style::default()),
        (tick_icon, Style::default().fg(p_col).add_modifier(Modifier::BOLD)),
        ("  ", Style::default()),
        (&chg_s, Style::default().fg(p_col).add_modifier(Modifier::BOLD)),
    ];
    render_spans(buf, x, *row, hero_spans);
    *row += 1;

    // 2. 24h range
    let r_bar = render_range_bar(st.last_price, st.low_24h, st.high_24h, 16);
    let low_s = format_comma(st.low_24h, 1);
    let high_s = format_comma(st.high_24h, 1);
    let range_spans: &[(&str, Style)] = &[
        ("  24h: ", lbl_style),
        (&low_s, Style::default().fg(Color::Green)),
        (" ", Style::default()),
        (&r_bar, Style::default().fg(Color::Cyan)),
        (" ", Style::default()),
        (&high_s, Style::default().fg(Color::Red)),
    ];
    render_spans(buf, x, *row, range_spans);
    *row += 1;

    // 3. Sparkline
    let closes: Vec<f64> = st.klines.iter().rev().take(25).map(|k| k.close).collect();
    if closes.len() >= 2 {
        let spark = generate_sparkline(&closes);
        let min_p = closes.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_p = closes.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let spark_col = if closes.last().unwrap_or(&0.0) >= closes.first().unwrap_or(&0.0) { Color::Green } else { Color::Red };
        let min_s = format_comma(min_p, 1);
        let max_s = format_comma(max_p, 1);
        let spark_spans: &[(&str, Style)] = &[
            ("  Trend(25m): ", lbl_style),
            (&spark, Style::default().fg(spark_col)),
            (" [", lbl_style),
            (&min_s, Style::default().fg(Color::Gray)),
            (" ~ ", lbl_style),
            (&max_s, Style::default().fg(Color::Gray)),
            ("]", lbl_style),
        ];
        render_spans(buf, x, *row, spark_spans);
        *row += 1;
    }

    // 4. Multi-Exchange Arbitrage
    if st.binance_price > 0.0 {
        let arb = st.last_price - st.binance_price;
        let arb_sign = if arb >= 0.0 { "+" } else { "" };
        let arb_col = if arb >= 0.0 { Color::Green } else { Color::Red };
        let byb_s = format_comma(st.last_price, 1);
        let bin_s = format_comma(st.binance_price, 1);
        let arb_s = format!("({}{:.1})", arb_sign, arb);
        let arb_spans: &[(&str, Style)] = &[
            ("  Multi-Exchange Arb: Bybit $", lbl_style),
            (&byb_s, Style::default().fg(Color::Cyan)),
            (" │ Bin: $", lbl_style),
            (&bin_s, Style::default().fg(Color::Yellow)),
            (" ", Style::default()),
            (&arb_s, Style::default().fg(arb_col).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, arb_spans);
        *row += 1;
    }

    // 5. Confluence Signal Engine (Cyan Accent)
    if st.settings.show_signal && *row < max_y - 2 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, "4-LAYER CONFLUENCE SIGNAL", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        *row += 1;

        let (score, bias, setup) = st.evaluate_confluence();
        let gauge = render_score_gauge(score, 14);
        let score_sign = if score >= 0 { "+" } else { "" };
        let score_col = if score > 15 { Color::Green } else if score < -15 { Color::Red } else { Color::Yellow };
        let sc_str = format!("{}{:>3}", score_sign, score);

        let score_spans: &[(&str, Style)] = &[
            ("  Score: [", lbl_style),
            (&gauge, Style::default().fg(Color::Cyan)),
            ("] ", lbl_style),
            (&sc_str, Style::default().fg(score_col).add_modifier(Modifier::BOLD)),
            (" │ ", lbl_style),
            (&bias, Style::default().fg(score_col).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, score_spans);
        *row += 1;

        let setup_col = if setup.contains("LONG") || setup.contains("BULL") {
            Color::LightGreen
        } else if setup.contains("SHORT") || setup.contains("BEAR") {
            Color::LightRed
        } else {
            Color::Yellow
        };
        let setup_spans: &[(&str, Style)] = &[
            ("  Setup: ", lbl_style),
            (&setup, Style::default().fg(setup_col).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, setup_spans);
        *row += 1;
    }

    // 6. Technical Signals (Sky Blue Accent)
    if st.settings.show_technical && *row < max_y - 2 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, "TECHNICAL SIGNALS (1-MIN)", Style::default().fg(Color::Rgb(100, 190, 255)).add_modifier(Modifier::BOLD));
        *row += 1;

        let rsi = st.calculate_rsi(14);
        let ema9 = st.calculate_ema(9);
        let ema21 = st.calculate_ema(21);
        let ema_trend = if ema9 > ema21 { "BULLISH ▲" } else { "BEARISH ▼" };
        let ema_col = if ema9 > ema21 { Color::Green } else { Color::Red };
        let rsi_col = if rsi < 30.0 { Color::Green } else if rsi > 70.0 { Color::Red } else { Color::Cyan };
        let rsi_raw = format!("{:.1}", rsi);
        let rsi_s = format!("{:<19}", rsi_raw);

        let t1_spans: &[(&str, Style)] = &[
            ("  • RSI(14) : ", lbl_style),
            (&rsi_s, Style::default().fg(rsi_col).add_modifier(Modifier::BOLD)),
            ("│  • EMA Trend: ", lbl_style),
            (ema_trend, Style::default().fg(ema_col).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, t1_spans);
        *row += 1;

        let ema9_raw = format!("${}", format_comma(ema9, 2));
        let ema9_s = format!("{:<19}", ema9_raw);
        let ema21_s = format!("${}", format_comma(ema21, 2));
        let t2_spans: &[(&str, Style)] = &[
            ("  • EMA(9)  : ", lbl_style),
            (&ema9_s, Style::default().fg(Color::LightGreen)),
            ("│  • EMA(21) : ", lbl_style),
            (&ema21_s, Style::default().fg(Color::Yellow)),
        ];
        render_spans(buf, x, *row, t2_spans);
        *row += 1;
    }

    // 7. Whale Radar (Gold / Amber Accent)
    if st.settings.show_whale_radar && *row < max_y - 2 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, &format!("WHALE & LARGE TRADE RADAR (≥ {:.1} BTC)", st.settings.whale_threshold), Style::default().fg(Color::Rgb(255, 215, 0)).add_modifier(Modifier::BOLD));
        *row += 1;

        let max_w = if max_y.saturating_sub(*row) > 10 { 3 } else { 2 };
        if !st.large_trades.is_empty() {
            for tr in st.large_trades.iter().take(max_w) {
                let dt = Local.timestamp_opt(tr.ts as i64, 0).unwrap();
                let t_str = dt.format("%H:%M:%S").to_string();
                let dot = if tr.side == "Buy" { "🟢" } else { "🔴" };
                let side_col = if tr.side == "Buy" { Color::Green } else { Color::Red };
                let vol_s = format!("{:>6.2} BTC", tr.vol);
                let usd_s = format!("({})", format_usd(tr.usd));
                let px_s = format!("${}", format_comma(tr.price, 1));
                let tr_spans: &[(&str, Style)] = &[
                    ("  ", Style::default()),
                    (&t_str, lbl_style),
                    (" ", Style::default()),
                    (dot, Style::default()),
                    (" ", Style::default()),
                    (&tr.side, Style::default().fg(side_col).add_modifier(Modifier::BOLD)),
                    ("  ", Style::default()),
                    (&vol_s, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    (" ", Style::default()),
                    (&usd_s, Style::default().fg(Color::White)),
                    (" @ ", lbl_style),
                    (&px_s, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ];
                render_spans(buf, x, *row, tr_spans);
                *row += 1;
            }
        } else {
            buf.set_string(x + 2, *row, &format!("Listening for whale orders (≥ {:.1} BTC)... [Press 'w' to adjust]", st.settings.whale_threshold), lbl_style);
            *row += 1;
        }
    }

    // 8. Vacuum & Anomaly (Coral / Warning Accent)
    if st.settings.show_vacuum_radar && *row < max_y - 2 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, "ORDER FLOW ANOMALIES & LIQUIDITY VACUUM", Style::default().fg(Color::Rgb(255, 140, 60)).add_modifier(Modifier::BOLD));
        *row += 1;

        let now = now_secs();
        let (vac_str, vac_style) = if let Some(ref vac) = st.vacuum_alert {
            if now - st.vacuum_ts < 12.0 {
                (vac.as_str(), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            } else {
                (st.vacuum_status.as_str(), Style::default().fg(Color::Green))
            }
        } else {
            (st.vacuum_status.as_str(), Style::default().fg(Color::Green))
        };
        let vac_spans: &[(&str, Style)] = &[
            ("  • Vacuum : 🟢 ", lbl_style),
            (vac_str, vac_style),
        ];
        render_spans(buf, x, *row, vac_spans);
        *row += 1;

        let (anom_str, anom_style) = if let Some(ref anom) = st.latest_anomaly {
            if now - st.anomaly_ts < 12.0 {
                (anom.as_str(), Style::default().fg(Color::Rgb(255, 140, 0)).add_modifier(Modifier::BOLD))
            } else {
                ("Normal Flow (No Active Spikes)", Style::default().fg(Color::Green))
            }
        } else {
            ("Normal Flow (No Active Spikes)", Style::default().fg(Color::Green))
        };
        let anom_spans: &[(&str, Style)] = &[
            ("  • Anomaly: 🟢 ", lbl_style),
            (anom_str, anom_style),
        ];
        render_spans(buf, x, *row, anom_spans);
        *row += 1;
    }

    // 9. Orderbook Microstructure & Pressure (Mint Green Accent)
    if st.settings.show_microstructure && *row < max_y - 2 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, "ORDERBOOK MICROSTRUCTURE & PRESSURE", Style::default().fg(Color::Rgb(80, 240, 160)).add_modifier(Modifier::BOLD));
        *row += 1;

        let (bid_ratio, _, _) = st.get_orderbook_pressure(20);
        let (b_bar, a_bar) = render_pressure_bar(bid_ratio, 12);
        let best_bid = st.bids.keys().next_back().map(|k| from_key(*k)).unwrap_or(0.0);
        let bid_sz = st.bids.values().next_back().copied().unwrap_or(0.0);
        let best_ask = st.asks.keys().next().map(|k| from_key(*k)).unwrap_or(0.0);
        let ask_sz = st.asks.values().next().copied().unwrap_or(0.0);
        let spread = (best_ask - best_bid).max(0.0);
        let basis = st.last_price - st.index_price;

        let b_pct_s = format!("{:.0}% Bid", bid_ratio);
        let a_pct_s = format!("{:.0}% Ask", 100.0 - bid_ratio);
        let press_spans: &[(&str, Style)] = &[
            ("  Pressure: [", lbl_style),
            (&b_bar, Style::default().fg(Color::Green)),
            (&a_bar, Style::default().fg(Color::Red)),
            ("] ", lbl_style),
            (&b_pct_s, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            (" vs ", lbl_style),
            (&a_pct_s, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, press_spans);
        *row += 1;

        let bid_raw = format!("${} ({:.2})", format_comma(best_bid, 2), bid_sz);
        let bid_val_s = format!("{:<19}", bid_raw);
        let ask_val_s = format!("${} ({:.2})", format_comma(best_ask, 2), ask_sz);
        let ob1_spans: &[(&str, Style)] = &[
            ("  • Best Bid: ", lbl_style),
            (&bid_val_s, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
            ("│  • Best Ask: ", lbl_style),
            (&ask_val_s, Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, ob1_spans);
        *row += 1;

        let spread_pct = if best_bid > 0.0 { spread / best_bid * 100.0 } else { 0.0 };
        let spr_raw = format!("${:.2} ({:.4}%)", spread, spread_pct);
        let spr_s = format!("{:<19}", spr_raw);
        let basis_s = format!("{:+.2}", basis);
        let basis_col = if basis >= 0.0 { Color::Green } else { Color::Red };
        let ob2_spans: &[(&str, Style)] = &[
            ("  • Spread  : ", lbl_style),
            (&spr_s, Style::default().fg(Color::Yellow)),
            ("│  • Basis   : ", lbl_style),
            (&basis_s, Style::default().fg(basis_col).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, ob2_spans);
        *row += 1;

        // Nearest Liquidity Walls
        if *row < max_y - 2 {
            let (_, _, bid_wall, ask_wall) = st.get_ladder_rows(st.settings.ladder_step, 5);
            let bw_raw = format!("${} ({:.1} BTC)", format_comma(bid_wall.0, 0), bid_wall.1);
            let bw_s = format!("{:<19}", bw_raw);
            let aw_s = format!("${} ({:.1} BTC)", format_comma(ask_wall.0, 0), ask_wall.1);
            let walls_spans: &[(&str, Style)] = &[
                ("  • Bid Wall: ", lbl_style),
                (&bw_s, Style::default().fg(Color::LightGreen)),
                ("│  • Ask Wall: ", lbl_style),
                (&aw_s, Style::default().fg(Color::LightRed)),
            ];
            render_spans(buf, x, *row, walls_spans);
            *row += 1;
        }
    }

    // 10. Derivatives & Metrics (Lavender Accent)
    if st.settings.show_derivatives && *row < max_y - 2 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, "DERIVATIVES & MARKET METRICS", Style::default().fg(Color::Rgb(200, 150, 255)).add_modifier(Modifier::BOLD));
        *row += 1;

        let fund_s = format!("{:+.4}%", st.funding_rate);
        let fund_col = if st.funding_rate >= 0.0 { Color::Green } else { Color::Red };
        let vol_s = format!("{} BTC", format_vol(st.volume_24h));
        let m1_spans: &[(&str, Style)] = &[
            ("  • 24h Vol : ", lbl_style),
            (&vol_s, Style::default().fg(Color::White)),
            ("       │  • Funding : ", lbl_style),
            (&fund_s, Style::default().fg(fund_col).add_modifier(Modifier::BOLD)),
        ];
        render_spans(buf, x, *row, m1_spans);
        *row += 1;

        let turn_s = format!("{:<14}", format_usd(st.turnover_24h));
        let mark_s = format!("${}", format_comma(st.mark_price, 2));
        let m2_spans: &[(&str, Style)] = &[
            ("  • 24h Turn: ", lbl_style),
            (&turn_s, Style::default().fg(Color::White)),
            ("│  • Mark Px : ", lbl_style),
            (&mark_s, Style::default().fg(Color::Cyan)),
        ];
        render_spans(buf, x, *row, m2_spans);
        *row += 1;

        let oi_val_s = format!("{:<14}", format_usd(st.open_interest_val));
        let oi_qty_s = format!("{} BTC", format_vol(st.open_interest));
        let m3_spans: &[(&str, Style)] = &[
            ("  • OI Value: ", lbl_style),
            (&oi_val_s, Style::default().fg(Color::Yellow)),
            ("│  • OI Qty  : ", lbl_style),
            (&oi_qty_s, Style::default().fg(Color::White)),
        ];
        render_spans(buf, x, *row, m3_spans);
        *row += 1;
    }

    // 11. Candles (1m)
    if st.settings.show_candles && *row < max_y - 3 {
        draw_horizontal_divider(x, *row, inner_w, border_col, buf);
        *row += 1;
        buf.set_string(x + 2, *row, "LATEST 1-MIN CANDLES", Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        *row += 1;
        buf.set_string(x + 2, *row, "Time     Open        High        Low         Close       Dir", lbl_style);
        *row += 1;

        let max_candles = ((max_y.saturating_sub(*row + 1)) as usize).clamp(3, 5);
        for k in st.klines.iter().take(max_candles) {
            let dt = Local.timestamp_opt(k.start / 1000, 0).unwrap();
            let t_str = dt.format("%H:%M").to_string();
            let dir = if k.close >= k.open { "▲" } else { "▼" };
            let dir_col = if k.close >= k.open { Color::LightGreen } else { Color::LightRed };
            let o_s = format!("{:<11.1}", k.open);
            let h_s = format!("{:<11.1}", k.high);
            let l_s = format!("{:<11.1}", k.low);
            let c_s = format!("{:<11.1}", k.close);
            let c_line: &[(&str, Style)] = &[
                ("  ", Style::default()),
                (&t_str, lbl_style),
                ("   ", Style::default()),
                (&o_s, Style::default().fg(Color::Gray)),
                (&h_s, Style::default().fg(Color::Gray)),
                (&l_s, Style::default().fg(Color::Gray)),
                (&c_s, Style::default().fg(dir_col).add_modifier(Modifier::BOLD)),
                (" ", Style::default()),
                (dir, Style::default().fg(dir_col).add_modifier(Modifier::BOLD)),
            ];
            render_spans(buf, x, *row, c_line);
            *row += 1;
        }
    }
}

fn render_ladder_page(st: &MarketState, inner_w: usize, x: u16, row: &mut u16, max_y: u16, border_col: Color, buf: &mut Buffer) {
    if *row >= max_y { return; }

    let lbl_style = Style::default().fg(Color::Rgb(160, 175, 195));

    let best_bid = st.bids.keys().next_back().map(|k| from_key(*k)).unwrap_or(0.0);
    let best_ask = st.asks.keys().next().map(|k| from_key(*k)).unwrap_or(0.0);
    let spread = (best_ask - best_bid).max(0.0);
    let basis = st.last_price - st.index_price;

    let p_col = if st.last_price >= st.prev_price { Color::LightGreen } else { Color::LightRed };
    let px_s = format!("${} ", format_comma(st.last_price, 2));
    let spr_s = format!("${:.2} ", spread);
    let basis_s = format!("{:+.2} ", basis);
    let step_s = format!("${:.0} ", st.settings.ladder_step);
    let depth_s = format!("{} ", st.settings.ladder_rows);

    let hero_spans: &[(&str, Style)] = &[
        ("  BTCUSDT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        (&px_s, Style::default().fg(p_col).add_modifier(Modifier::BOLD)),
        ("│ Spr: ", lbl_style),
        (&spr_s, Style::default().fg(Color::Yellow)),
        ("│ Basis: ", lbl_style),
        (&basis_s, Style::default().fg(if basis >= 0.0 { Color::Green } else { Color::Red })),
        ("│ Step: ", lbl_style),
        (&step_s, Style::default().fg(Color::Yellow)),
        ("│ Depth: ", lbl_style),
        (&depth_s, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ];
    render_spans(buf, x, *row, hero_spans);
    *row += 1;

    let (bid_ratio, b_vol, a_vol) = st.get_orderbook_pressure(20);
    let (b_bar, a_bar) = render_pressure_bar(bid_ratio, 12);
    let b_pct_s = format!("{:.0}% Bid ", bid_ratio);
    let a_pct_s = format!("{:.0}% Ask ", 100.0 - bid_ratio);
    let tot_s = format!("({:.0} BTC)", b_vol + a_vol);
    let depth_spans: &[(&str, Style)] = &[
        ("  Depth Flow: [", lbl_style),
        (&b_bar, Style::default().fg(Color::Green)),
        (&a_bar, Style::default().fg(Color::Red)),
        ("] ", lbl_style),
        (&b_pct_s, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ("vs ", lbl_style),
        (&a_pct_s, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        (&tot_s, lbl_style),
    ];
    render_spans(buf, x, *row, depth_spans);
    *row += 1;

    draw_horizontal_divider(x, *row, inner_w, border_col, buf);
    *row += 1;

    let header_title = format!("BOOKMAP DEPTH MATRIX & HEATMAP LADDER ({} Rows)", st.settings.ladder_rows);
    buf.set_string(x + 2, *row, &header_title, Style::default().fg(Color::Rgb(255, 200, 50)).add_modifier(Modifier::BOLD));
    *row += 1;

    buf.set_string(x + 2, *row, "Price        Size(BTC)   Depth Heatmap             Cumul(BTC)", lbl_style);
    *row += 1;

    // Calculate maximum depth levels that can safely fit inside the terminal
    let available_ladder_space = max_y.saturating_sub(*row + 6) as usize;
    let max_possible_depth = available_ladder_space.saturating_sub(1) / 2;
    let depth_rows = st.settings.ladder_rows.min(max_possible_depth).max(3);

    let (asks, bids, bid_wall, ask_wall) = st.get_ladder_rows(st.settings.ladder_step, depth_rows);
    let max_sz = asks.iter().chain(bids.iter()).map(|r| r.size).fold(1.0f64, f64::max);

    // Asks
    for r in &asks {
        let is_wall = r.is_wall || (r.size >= max_sz * 0.7 && r.size >= 4.0);
        let bar_len = ((r.size / max_sz).clamp(0.0, 1.0) * 16.0).round() as usize;
        let bar_len = bar_len.max(if r.size > 0.05 { 1 } else { 0 });
        let filled_bar = "█".repeat(bar_len);
        let empty_bar = "░".repeat(16 - bar_len);
        let wall_tag = if is_wall { " WALL " } else { "      " };

        let bar_col = if is_wall {
            Color::Rgb(255, 215, 0)
        } else if r.size >= max_sz * 0.35 {
            Color::Rgb(255, 140, 0)
        } else {
            Color::Red
        };

        let price_str = format!("  {:<10} ", format_comma(r.price, 1));
        let size_str = format!("{:>6.2} BTC ", r.size);
        let wall_str = wall_tag;
        let cumul_str = format!("{:>8.2} ", r.cumulative);

        let row_spans: &[(&str, Style)] = &[
            (&price_str, if is_wall { Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::LightRed) }),
            (&size_str, if is_wall { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
            ("[", lbl_style),
            (&filled_bar, Style::default().fg(bar_col)),
            (&empty_bar, Style::default().fg(Color::Rgb(50, 50, 50))),
            ("]", lbl_style),
            (wall_str, Style::default().fg(Color::Rgb(255, 215, 0)).add_modifier(Modifier::BOLD)),
            (&cumul_str, Style::default().fg(Color::Gray)),
            ("▼", Style::default().fg(Color::Red)),
        ];
        render_spans(buf, x, *row, row_spans);
        *row += 1;
    }

    // Mid price line
    let mid_tag = format!(" $ {} CURRENT MID PRICE ", format_comma(st.last_price, 2));
    let tag_len = mid_tag.chars().count();
    let left_dash = (inner_w.saturating_sub(tag_len)) / 2;
    let right_dash = inner_w.saturating_sub(tag_len + left_dash);
    let left_s = "─".repeat(left_dash);
    let right_s = "─".repeat(right_dash);

    let mid_spans: &[(&str, Style)] = &[
        (&left_s, Style::default().fg(Color::Cyan)),
        (&mid_tag, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        (&right_s, Style::default().fg(Color::Cyan)),
    ];
    render_spans(buf, x, *row, mid_spans);
    *row += 1;

    // Bids
    for r in &bids {
        let is_wall = r.is_wall || (r.size >= max_sz * 0.7 && r.size >= 4.0);
        let bar_len = ((r.size / max_sz).clamp(0.0, 1.0) * 16.0).round() as usize;
        let bar_len = bar_len.max(if r.size > 0.05 { 1 } else { 0 });
        let filled_bar = "█".repeat(bar_len);
        let empty_bar = "░".repeat(16 - bar_len);
        let wall_tag = if is_wall { " WALL " } else { "      " };

        let bar_col = if is_wall {
            Color::Rgb(0, 255, 255)
        } else if r.size >= max_sz * 0.35 {
            Color::LightGreen
        } else {
            Color::Green
        };

        let price_str = format!("  {:<10} ", format_comma(r.price, 1));
        let size_str = format!("{:>6.2} BTC ", r.size);
        let wall_str = wall_tag;
        let cumul_str = format!("{:>8.2} ", r.cumulative);

        let row_spans: &[(&str, Style)] = &[
            (&price_str, if is_wall { Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::LightGreen) }),
            (&size_str, if is_wall { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
            ("[", lbl_style),
            (&filled_bar, Style::default().fg(bar_col)),
            (&empty_bar, Style::default().fg(Color::Rgb(50, 50, 50))),
            ("]", lbl_style),
            (wall_str, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            (&cumul_str, Style::default().fg(Color::Gray)),
            ("▲", Style::default().fg(Color::Green)),
        ];
        render_spans(buf, x, *row, row_spans);
        *row += 1;
    }

    draw_horizontal_divider(x, *row, inner_w, border_col, buf);
    *row += 1;

    buf.set_string(x + 2, *row, "ORDER FLOW & LIQUIDITY WALLS (CVD)", Style::default().fg(Color::Rgb(255, 200, 50)).add_modifier(Modifier::BOLD));
    *row += 1;

    let bw_s = format!("${} ", format_comma(bid_wall.0, 0));
    let bw_btc = format!("({:.1} BTC) ", bid_wall.1);
    let aw_s = format!("${} ", format_comma(ask_wall.0, 0));
    let aw_btc = format!("({:.1} BTC)", ask_wall.1);
    let walls_spans: &[(&str, Style)] = &[
        ("  ▲ Wall: ", lbl_style),
        (&bw_s, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        (&bw_btc, Style::default().fg(Color::Cyan)),
        ("│ ▼ Wall: ", lbl_style),
        (&aw_s, Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
        (&aw_btc, Style::default().fg(Color::Yellow)),
    ];
    render_spans(buf, x, *row, walls_spans);
    *row += 1;

    // Alert Banner
    let now = now_secs();
    if let Some(ref w) = st.latest_whale {
        if now - st.whale_ts < 8.0 {
            let dot = if w.side == "Buy" { "🟢" } else { "🔴" };
            let side_col = if w.side == "Buy" { Color::Green } else { Color::Red };
            let vol_s = format!("{:.2} BTC ", w.vol);
            let usd_s = format!("({}) ", format_usd(w.usd));
            let px_s = format!("${}", format_comma(w.price, 1));
            let w_spans: &[(&str, Style)] = &[
                ("  ", Style::default()),
                (dot, Style::default()),
                (" [WHALE ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (&w.side, Style::default().fg(side_col).add_modifier(Modifier::BOLD)),
                ("]: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (&vol_s, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                (&usd_s, Style::default().fg(Color::White)),
                ("@ ", lbl_style),
                (&px_s, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ];
            render_spans(buf, x, *row, w_spans);
        } else {
            render_secondary_alert(st, now, x, *row, buf);
        }
    } else {
        render_secondary_alert(st, now, x, *row, buf);
    }
    *row += 1;

    // Session CVD
    let b_vol = st.buy_vol_session;
    let s_vol = st.sell_vol_session;
    let delta = b_vol - s_vol;
    let total = b_vol + s_vol;
    let cvd_ratio = if total > 0.0 { (b_vol / total) * 100.0 } else { 50.0 };
    let (b_b, a_b) = render_pressure_bar(cvd_ratio, 12);
    let delta_col = if delta >= 0.0 { Color::Green } else { Color::Red };
    let delta_sign = if delta >= 0.0 { "+" } else { "" };
    let b_s = format!("{:.1}", b_vol);
    let s_s = format!("{:.1} ", s_vol);
    let d_s = format!("{}{:.1} BTC", delta_sign, delta);

    let cvd_spans: &[(&str, Style)] = &[
        ("  Session CVD: [", lbl_style),
        (&b_b, Style::default().fg(Color::Green)),
        (&a_b, Style::default().fg(Color::Red)),
        ("] ", lbl_style),
        (&b_s, Style::default().fg(Color::Green)),
        ("/", lbl_style),
        (&s_s, Style::default().fg(Color::Red)),
        ("│ Delta: ", lbl_style),
        (&d_s, Style::default().fg(delta_col).add_modifier(Modifier::BOLD)),
    ];
    render_spans(buf, x, *row, cvd_spans);
    *row += 1;
}

fn render_secondary_alert(st: &MarketState, now: f64, x: u16, y: u16, buf: &mut Buffer) {
    let lbl_style = Style::default().fg(Color::Rgb(160, 175, 195));
    if let Some(ref anom) = st.latest_anomaly {
        if now - st.anomaly_ts < 10.0 {
            buf.set_string(x + 2, y, anom, Style::default().fg(Color::Rgb(255, 140, 0)).add_modifier(Modifier::BOLD));
            return;
        }
    }
    if let Some(ref vac) = st.vacuum_alert {
        if now - st.vacuum_ts < 10.0 {
            buf.set_string(x + 2, y, vac, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            return;
        }
    }
    if let Some(ref spoof) = st.latest_spoof {
        if now - st.spoof_ts < 10.0 {
            buf.set_string(x + 2, y, &format!("🟠 [SPOOF] {} [WALL CANCELLED]", spoof), Style::default().fg(Color::Rgb(255, 140, 0)));
            return;
        }
    }
    if let Some(ref liq) = st.latest_liq {
        if now - st.liq_ts < 15.0 {
            buf.set_string(x + 2, y, &format!("🔴 [LIQUIDATION] {}", liq), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            return;
        }
    }
    let l_s = format!("${:.0}K ", st.long_liq_usd / 1000.0);
    let s_s = format!("${:.0}K", st.short_liq_usd / 1000.0);
    let rekt_spans: &[(&str, Style)] = &[
        ("  Session REKT: ", lbl_style),
        ("Long: ", lbl_style),
        (&l_s, Style::default().fg(Color::LightRed)),
        ("│ Short: ", lbl_style),
        (&s_s, Style::default().fg(Color::LightGreen)),
    ];
    render_spans(buf, x, y, rekt_spans);
}

fn render_heatmap_page(st: &MarketState, inner_w: usize, x: u16, row: &mut u16, max_y: u16, border_col: Color, buf: &mut Buffer) {
    if *row >= max_y { return; }

    let lbl_style = Style::default().fg(Color::Rgb(160, 175, 195));

    let best_bid = st.bids.keys().next_back().map(|k| from_key(*k)).unwrap_or(0.0);
    let best_ask = st.asks.keys().next().map(|k| from_key(*k)).unwrap_or(0.0);
    let spread = (best_ask - best_bid).max(0.0);
    let delta = st.buy_vol_session - st.sell_vol_session;

    let p_col = if st.last_price >= st.prev_price { Color::LightGreen } else { Color::LightRed };
    let px_s = format!("${} ", format_comma(st.last_price, 2));
    let spr_s = format!("${:.2} ", spread);
    let delta_s = format!("{:+.1} BTC ", delta);
    let step_s = format!("${:.0} ", st.settings.ladder_step);
    let depth_s = format!("{} ", st.settings.ladder_rows);

    let hero_spans: &[(&str, Style)] = &[
        ("  BTCUSDT ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        (&px_s, Style::default().fg(p_col).add_modifier(Modifier::BOLD)),
        ("│ Spr: ", lbl_style),
        (&spr_s, Style::default().fg(Color::Yellow)),
        ("│ CVD: ", lbl_style),
        (&delta_s, Style::default().fg(if delta >= 0.0 { Color::Green } else { Color::Red })),
        ("│ Step: ", lbl_style),
        (&step_s, Style::default().fg(Color::Yellow)),
        ("│ Depth: ", lbl_style),
        (&depth_s, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ];
    render_spans(buf, x, *row, hero_spans);
    *row += 1;

    let legend_spans: &[(&str, Style)] = &[
        ("  Wall: ", lbl_style),
        ("█>20B ", Style::default().fg(Color::Magenta)),
        ("█10B ", Style::default().fg(Color::Rgb(255, 140, 0))),
        ("█5B ", Style::default().fg(Color::Cyan)),
        ("│ Flow: ", lbl_style),
        ("●Buy ", Style::default().fg(Color::Green)),
        ("●Sell ", Style::default().fg(Color::Red)),
        ("──Price", Style::default().fg(Color::Yellow)),
    ];
    render_spans(buf, x, *row, legend_spans);
    *row += 1;

    draw_horizontal_divider(x, *row, inner_w, border_col, buf);
    *row += 1;

    let header_title = format!("   Price │        ◄── BOOKMAP 2D TIME-SERIES ({} Rows) ──►        │ Depth", st.settings.ladder_rows * 2);
    buf.set_string(x + 2, *row, &header_title, Style::default().fg(Color::Rgb(215, 100, 255)).add_modifier(Modifier::BOLD));
    *row += 1;

    let step = st.settings.ladder_step.max(1.0);
    let center_p = (st.last_price / step).round() * step;

    // Adapt number of tiers to available vertical space or configured rows
    let available_heatmap_space = max_y.saturating_sub(*row + 6) as usize;
    let requested_tiers = st.settings.ladder_rows * 2;
    let num_tiers = requested_tiers.min(available_heatmap_space).max(6);
    let half_tiers = (num_tiers / 2) as i32;

    let time_cols = 20; // 20 intervals
    let slices: Vec<&crate::types::HistorySlice> = st.history_slices.iter().rev().take(time_cols).collect();

    for tier_idx in 0..num_tiers {
        let p = center_p + ((half_tiers - tier_idx as i32) as f64) * step;
        let is_curr = (p - center_p).abs() < (step * 0.5);
        let p_tag = if is_curr { "►" } else { " " };
        let p_col = if is_curr {
            Color::Yellow
        } else if p > st.last_price {
            Color::LightRed
        } else {
            Color::LightGreen
        };

        let mut row_chars: Vec<(char, Style)> = Vec::with_capacity(time_cols * 2);

        // Calculate depth at price p in latest snapshot
        let latest_depth: f64 = if p >= st.last_price {
            st.asks.iter()
                .filter(|(k, _)| (from_key(**k) - p).abs() < (step * 0.5))
                .map(|(_, s)| *s)
                .sum()
        } else {
            st.bids.iter()
                .filter(|(k, _)| (from_key(**k) - p).abs() < (step * 0.5))
                .map(|(_, s)| *s)
                .sum()
        };

        for col_idx in 0..time_cols {
            let slice_idx = time_cols.saturating_sub(1 + col_idx);
            let slice = slices.get(slice_idx);

            let (depth, has_buy, has_sell, has_price) = if let Some(sl) = slice {
                let d = if p >= sl.price {
                    sl.asks.iter()
                        .filter(|(pk, _)| (*pk - p).abs() < (step * 0.5))
                        .map(|(_, s)| *s)
                        .sum()
                } else {
                    sl.bids.iter()
                        .filter(|(pk, _)| (*pk - p).abs() < (step * 0.5))
                        .map(|(_, s)| *s)
                        .sum()
                };
                let hb = sl.trades.iter().any(|(tp, _, s)| (*tp - p).abs() < (step * 0.5) && s == "Buy");
                let hs = sl.trades.iter().any(|(tp, _, s)| (*tp - p).abs() < (step * 0.5) && s == "Sell");
                let hp = (sl.price - p).abs() < (step * 0.5);
                (d, hb, hs, hp)
            } else {
                (0.0, false, false, false)
            };

            let (ch, style) = if has_buy {
                ('●', Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else if has_sell {
                ('●', Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            } else if has_price {
                ('─', Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else if depth >= 20.0 {
                ('█', Style::default().fg(Color::Magenta))
            } else if depth >= 10.0 {
                ('█', Style::default().fg(Color::Rgb(255, 140, 0)))
            } else if depth >= 5.0 {
                ('█', Style::default().fg(Color::Cyan))
            } else if depth >= 2.0 {
                ('▓', Style::default().fg(Color::Blue))
            } else if depth >= 0.5 {
                ('▒', Style::default().fg(Color::DarkGray))
            } else {
                (' ', Style::default())
            };

            row_chars.push((ch, style));
            row_chars.push((if has_price { '─' } else { ' ' }, style));
        }

        let p_label = format!(" {} ${:<7} │", p_tag, format_comma(p, 0));
        buf.set_string(x + 2, *row, &p_label, Style::default().fg(p_col).add_modifier(if is_curr { Modifier::BOLD } else { Modifier::empty() }));

        let mut start_col = x + 15;
        for (ch, st_c) in row_chars {
            buf.set_string(start_col, *row, &ch.to_string(), st_c);
            start_col += 1;
        }

        let d_label = format!("│ {:>4.1}B", latest_depth);
        let d_col = if is_curr {
            Color::Yellow
        } else if p > st.last_price {
            if latest_depth >= 15.0 { Color::Rgb(255, 215, 0) }
            else if latest_depth >= 8.0 { Color::Rgb(255, 140, 0) }
            else { Color::LightRed }
        } else {
            if latest_depth >= 15.0 { Color::Cyan }
            else if latest_depth >= 8.0 { Color::LightGreen }
            else { Color::Green }
        };
        buf.set_string(start_col, *row, &d_label, Style::default().fg(d_col));
        *row += 1;
    }

    buf.set_string(x + 13, *row, "└──────T-20s─────T-15s─────T-10s─────T-5s───NOW┘", lbl_style);
    *row += 1;

    draw_horizontal_divider(x, *row, inner_w, border_col, buf);
    *row += 1;

    let (_, _, bid_wall, ask_wall) = st.get_ladder_rows(st.settings.ladder_step, 10);
    let bw_s = format!("${} ", format_comma(bid_wall.0, 0));
    let bw_btc = format!("({:.1} BTC) ", bid_wall.1);
    let aw_s = format!("${} ", format_comma(ask_wall.0, 0));
    let aw_btc = format!("({:.1} BTC)", ask_wall.1);
    let walls_spans: &[(&str, Style)] = &[
        ("  ▲ Wall: ", lbl_style),
        (&bw_s, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        (&bw_btc, Style::default().fg(Color::Cyan)),
        ("│ ▼ Wall: ", lbl_style),
        (&aw_s, Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
        (&aw_btc, Style::default().fg(Color::Yellow)),
    ];
    render_spans(buf, x, *row, walls_spans);
    *row += 1;

    let now = now_secs();
    render_secondary_alert(st, now, x, *row, buf);
    *row += 1;
}
