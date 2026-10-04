mod state;
mod types;
mod ui;
mod web;
mod ws;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use state::MarketState;
use types::Settings;
use std::{
    io::stdout,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use ui::TerminalView;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().collect();
    let mut initial_page = Settings::load().default_page;
    let mut open_menu = false;
    let mut run_once = false;
    let mut web_only = false;
    let mut interval_ms = 100u64;
    let mut custom_symbol: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dash" => initial_page = 1,
            "--book" => initial_page = 2,
            "--heat" => initial_page = 3,
            "--web" | "--headless" => web_only = true,
            "-s" | "--symbol" => {
                if i + 1 < args.len() {
                    custom_symbol = Some(args[i + 1].to_uppercase());
                    i += 1;
                }
            }
            "-p" | "--page" => {
                if i + 1 < args.len() {
                    if let Ok(p) = args[i + 1].parse::<usize>() {
                        initial_page = p.clamp(1, 3);
                    }
                    i += 1;
                }
            }
            "--menu" => open_menu = true,
            "--once" => run_once = true,
            "-i" => {
                if i + 1 < args.len() {
                    if let Ok(sec) = args[i + 1].parse::<f64>() {
                        interval_ms = (sec * 1000.0).max(10.0) as u64;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let state = Arc::new(RwLock::new(MarketState::new()));
    {
        let mut st = state.write().await;
        st.current_page = initial_page;
        st.is_menu_open = open_menu;
        if let Some(ref sym) = custom_symbol {
            st.reset_for_symbol(sym);
        }
    }

    // Initial seed from REST
    ws::seed_from_rest(state.clone()).await;

    // If run_once, render one frame directly to buffer and print line-by-line
    if run_once {
        use ratatui::widgets::Widget;
        let term_w = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| crossterm::terminal::size().map(|(w, _)| w).unwrap_or(110));
        let term_h = std::env::var("LINES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| crossterm::terminal::size().map(|(_, h)| h).unwrap_or(36));
        let width = term_w.max(68);
        let height = term_h.max(25);
        let area = ratatui::layout::Rect::new(0, 0, width, height);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        let st = state.read().await;
        TerminalView::new(&st).render(area, &mut buf);

        for y in 0..height {
            let mut line = String::new();
            for x in 0..width {
                let cell = &buf[(x, y)];
                line.push_str(cell.symbol());
            }
            if !line.trim().is_empty() {
                println!("{}", line);
            }
        }
        println!(" [Tab] Page │ [s] Symbol: {} │ [+/-] Step: ${:.1} │ [r] Depth: {} │ [w] Whale: ≥{:.1}{} │ [m] Menu │ [q] Exit", st.symbol, st.settings.ladder_step, st.settings.ladder_rows, st.settings.whale_threshold, st.asset_unit());
        return Ok(());
    }

    if web_only {
        println!("🚀 Starting BTC-Terminal Headless Web Server...");
        {
            let mut st = state.write().await;
            st.settings.enable_web_ui = true;
        }
        tokio::spawn(ws::run_bybit_ws(state.clone()));
        tokio::spawn(ws::run_binance_ws(state.clone()));
        let slice_state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(1000)).await;
                let mut st = slice_state.write().await;
                st.record_history_slice();
                st.evaluate_anomalies_and_vacuum();
            }
        });
        web::run_web_server(state.clone()).await;
        return Ok(());
    }

    // Spawn WebSocket tasks
    tokio::spawn(ws::run_bybit_ws(state.clone()));
    tokio::spawn(ws::run_binance_ws(state.clone()));

    // Spawn Web Server task
    tokio::spawn(web::run_web_server(state.clone()));

    // Periodic task for slices and anomaly detection
    let slice_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            let mut st = slice_state.write().await;
            st.record_history_slice();
            st.evaluate_anomalies_and_vacuum();
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    // Panic hook to guarantee terminal restoration
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Show);
        default_panic(info);
    }));

    let mut last_render = tokio::time::Instant::now();
    let render_interval = Duration::from_millis(interval_ms);

    loop {
        // Poll keyboard input
        if event::poll(Duration::from_millis(15))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        let mut st = state.write().await;

                        // Handle Ctrl+C or 'q'
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            break;
                        }
                        if !st.is_menu_open && key.code == KeyCode::Char('q') {
                            break;
                        }

                        if st.is_menu_open {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('q') => {
                                    st.is_menu_open = false;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    st.menu_idx = (st.menu_idx + 7) % 8;
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    st.menu_idx = (st.menu_idx + 1) % 8;
                                }
                                KeyCode::Enter | KeyCode::Char(' ') => {
                                    match st.menu_idx {
                                        0 => st.settings.show_signal = !st.settings.show_signal,
                                        1 => st.settings.show_technical = !st.settings.show_technical,
                                        2 => st.settings.show_whale_radar = !st.settings.show_whale_radar,
                                        3 => st.settings.show_vacuum_radar = !st.settings.show_vacuum_radar,
                                        4 => st.settings.show_microstructure = !st.settings.show_microstructure,
                                        5 => st.settings.show_derivatives = !st.settings.show_derivatives,
                                        6 => st.settings.show_candles = !st.settings.show_candles,
                                        7 => st.settings.enable_web_ui = !st.settings.enable_web_ui,
                                        _ => {}
                                    }
                                    st.settings.save();
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Tab => {
                                    st.current_page = if st.current_page >= 3 { 1 } else { st.current_page + 1 };
                                }
                                KeyCode::Char('1') => st.current_page = 1,
                                KeyCode::Char('2') => st.current_page = 2,
                                KeyCode::Char('3') => st.current_page = 3,
                                KeyCode::Char('m') => st.is_menu_open = true,
                                KeyCode::Char('s') | KeyCode::Char('S') => {
                                    let next_sym = match st.symbol.as_str() {
                                        "BTCUSDT" => "XAUUSDT",
                                        "XAUUSDT" => "ETHUSDT",
                                        "ETHUSDT" => "SOLUSDT",
                                        _ => "BTCUSDT",
                                    };
                                    st.reset_for_symbol(next_sym);
                                    let state_clone = state.clone();
                                    tokio::spawn(async move {
                                        crate::ws::seed_from_rest(state_clone).await;
                                    });
                                }
                                KeyCode::Char('+') | KeyCode::Char('=') => {
                                    st.settings.ladder_step = (st.settings.ladder_step + 1.0).min(50.0);
                                    st.settings.save();
                                }
                                KeyCode::Char('-') | KeyCode::Char('_') => {
                                    st.settings.ladder_step = (st.settings.ladder_step - 1.0).max(1.0);
                                    st.settings.save();
                                }
                                KeyCode::Char('w') | KeyCode::Char('W') => {
                                    let curr = st.settings.whale_threshold;
                                    st.settings.whale_threshold = if curr < 1.5 {
                                        2.0
                                    } else if curr < 3.0 {
                                        5.0
                                    } else if curr < 7.0 {
                                        10.0
                                    } else if curr < 15.0 {
                                        0.5
                                    } else {
                                        1.0
                                    };
                                    st.settings.save();
                                }
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    st.settings.ladder_rows = match st.settings.ladder_rows {
                                        5 => 8,
                                        8 => 10,
                                        10 => 15,
                                        15 => 20,
                                        _ => 5,
                                    };
                                    st.settings.save();
                                }
                                KeyCode::Char('[') => {
                                    st.settings.ladder_rows = st.settings.ladder_rows.saturating_sub(1).max(4);
                                    st.settings.save();
                                }
                                KeyCode::Char(']') => {
                                    st.settings.ladder_rows = (st.settings.ladder_rows + 1).min(30);
                                    st.settings.save();
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Event::Resize(_, _) => {
                    let st = state.read().await;
                    terminal.draw(|f| {
                        f.render_widget(TerminalView::new(&st), f.area());
                    })?;
                }
                _ => {}
            }
        }

        // Render Frame with Rate Throttling
        if last_render.elapsed() >= render_interval {
            let st = state.read().await;
            terminal.draw(|f| {
                f.render_widget(TerminalView::new(&st), f.area());
            })?;
            last_render = tokio::time::Instant::now();
        }

        tokio::task::yield_now().await;
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)?;
    terminal.show_cursor()?;

    Ok(())
}
