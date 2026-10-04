use crate::state::MarketState;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

pub async fn run_web_server(state: Arc<RwLock<MarketState>>) {
    let port = {
        let st = state.read().await;
        st.settings.web_port
    };
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            println!("[Web] Server listening on http://{}", addr);
            l
        }
        Err(e) => {
            eprintln!("[Web] Failed to bind to {}: {}", addr, e);
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((mut socket, _)) => {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req_str = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req_str.lines().next().unwrap_or("");
                    let mut parts = first_line.split_whitespace();
                    let method = parts.next().unwrap_or("GET");
                    let raw_path = parts.next().unwrap_or("/");
                    let path = raw_path.split('?').next().unwrap_or("/");

                    let is_enabled = {
                        let st = state_clone.read().await;
                        st.settings.enable_web_ui
                    };

                    if !is_enabled {
                        let disabled_html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Web Dashboard Disabled - btc-terminal</title>
    <style>
        body { background: #0b0e14; color: #e6edf3; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; text-align: center; }
        .card { background: #161b22; border: 1px solid #30363d; border-radius: 14px; padding: 40px; max-width: 500px; box-shadow: 0 16px 40px rgba(0,0,0,0.6); }
        .icon { font-size: 52px; margin-bottom: 16px; }
        h1 { margin: 0 0 14px 0; font-size: 24px; color: #f85149; }
        p { color: #8b949e; line-height: 1.6; margin: 0 0 24px 0; font-size: 15px; }
        code { background: #21262d; color: #58a6ff; padding: 4px 10px; border-radius: 6px; font-weight: bold; font-size: 14px; }
        .tip { font-size: 14px; color: #e3b341; background: rgba(227,179,65,0.12); border: 1px solid rgba(227,179,65,0.25); padding: 14px; border-radius: 8px; line-height: 1.5; }
    </style>
</head>
<body>
    <div class="card">
        <div class="icon">🛑</div>
        <h1>Web Dashboard is Disabled</h1>
        <p>The Web UI is currently turned <strong>OFF</strong> in the CLI terminal settings.</p>
        <div class="tip">
            👉 <strong>How to Enable:</strong> In the <code>btc-terminal</code> CLI window, press <code>m</code> to open the settings menu, navigate to <strong>Web Dashboard</strong>, and press <code>Enter</code> to toggle it to <code>[X]</code>.
        </div>
    </div>
</body>
</html>"#;
                        let resp = format!(
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            disabled_html.len(),
                            disabled_html
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                        return;
                    }

                    match (method, path) {
                        ("GET", "/") | ("GET", "/index.html") | ("HEAD", "/") | ("HEAD", "/index.html") => {
                            let html = include_str!("index.html");
                            let resp = if method == "HEAD" {
                                format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    html.len()
                                )
                            } else {
                                format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    html.len(),
                                    html
                                )
                            };
                            let _ = socket.write_all(resp.as_bytes()).await;
                        }
                        ("GET", "/api/klines") => {
                            let klines_json = {
                                let st = state_clone.read().await;
                                let mut k_list: Vec<_> = st.klines.iter().cloned().collect();
                                k_list.reverse();
                                serde_json::to_string(&k_list).unwrap_or_else(|_| "[]".to_string())
                            };
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                klines_json.len(),
                                klines_json
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                        }
                        ("GET", "/api/heatmap") => {
                            let slices_json = {
                                let st = state_clone.read().await;
                                let list: Vec<_> = st.history_slices.iter().collect();
                                serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
                            };
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                slices_json.len(),
                                slices_json
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                        }
                        ("GET", "/api/state") => {
                            let json_state = {
                                let st = state_clone.read().await;
                                let (score, bias, setup) = st.evaluate_confluence();
                                let (pressure_ratio, b_vol, a_vol) = st.get_orderbook_pressure(20);
                                let (asks_ladder, bids_ladder, bid_wall, ask_wall) = st.get_ladder_rows(st.settings.ladder_step, 15);
                                let (macro_bids, macro_asks) = st.get_macro_depth_buckets(2.5, 1500.0);
                                let bids_depth: Vec<[f64; 2]> = macro_bids.into_iter().map(|(p, s)| [p, s]).collect();
                                let asks_depth: Vec<[f64; 2]> = macro_asks.into_iter().map(|(p, s)| [p, s]).collect();

                                let asks_json: Vec<_> = asks_ladder.iter().map(|r| serde_json::json!({
                                    "price": r.price,
                                    "size": r.size,
                                    "cumulative": r.cumulative,
                                    "is_wall": r.is_wall
                                })).collect();

                                let bids_json: Vec<_> = bids_ladder.iter().map(|r| serde_json::json!({
                                    "price": r.price,
                                    "size": r.size,
                                    "cumulative": r.cumulative,
                                    "is_wall": r.is_wall
                                })).collect();

                                let rsi = st.calculate_rsi(14);
                                let ema9 = st.calculate_ema(9);
                                let ema21 = st.calculate_ema(21);

                                let best_bid = st.bids.keys().next_back().map(|k| crate::state::from_key(*k)).unwrap_or(0.0);
                                let best_ask = st.asks.keys().next().map(|k| crate::state::from_key(*k)).unwrap_or(0.0);
                                let spread = (best_ask - best_bid).max(0.0);
                                let arb_diff = if st.binance_price > 0.0 { st.last_price - st.binance_price } else { 0.0 };

                                let recent_trades: Vec<_> = st.trade_ticks.iter().rev().take(30).cloned().collect();
                                let recent_whales: Vec<_> = st.large_trades.iter().take(10).cloned().collect();

                                serde_json::json!({
                                    "last_price": st.last_price,
                                    "prev_price": st.prev_price,
                                    "price_change_24h": st.price_change_24h,
                                    "high_24h": st.high_24h,
                                    "low_24h": st.low_24h,
                                    "volume_24h": st.volume_24h,
                                    "turnover_24h": st.turnover_24h,
                                    "funding_rate": st.funding_rate,
                                    "next_funding_time": st.next_funding_time,
                                    "mark_price": st.mark_price,
                                    "index_price": st.index_price,
                                    "binance_price": st.binance_price,
                                    "arb_diff": arb_diff,
                                    "open_interest": st.open_interest,
                                    "open_interest_val": st.open_interest_val,
                                    "rsi": rsi,
                                    "ema9": ema9,
                                    "ema21": ema21,
                                    "confluence": {
                                        "score": score,
                                        "bias": bias,
                                        "setup": setup
                                    },
                                    "orderbook": {
                                        "best_bid": best_bid,
                                        "best_ask": best_ask,
                                        "spread": spread,
                                        "pressure_bid_ratio": pressure_ratio,
                                        "bid_vol": b_vol,
                                        "ask_vol": a_vol,
                                        "bid_wall_price": bid_wall.0,
                                        "bid_wall_size": bid_wall.1,
                                        "ask_wall_price": ask_wall.0,
                                        "ask_wall_size": ask_wall.1,
                                        "bids_depth": bids_depth,
                                        "asks_depth": asks_depth,
                                        "bids_ladder": bids_json,
                                        "asks_ladder": asks_json
                                    },
                                    "session_cvd": {
                                        "buy_vol": st.buy_vol_session,
                                        "sell_vol": st.sell_vol_session,
                                        "delta": st.buy_vol_session - st.sell_vol_session
                                    },
                                    "vacuum_status": st.vacuum_status,
                                    "latest_anomaly": st.latest_anomaly,
                                    "recent_trades": recent_trades,
                                    "recent_whales": recent_whales,
                                    "latest_slice": st.history_slices.back(),
                                    "enable_web_ui": st.settings.enable_web_ui,
                                    "web_port": st.settings.web_port
                                }).to_string()
                            };

                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                json_state.len(),
                                json_state
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                        }
                        _ => {
                            let not_found = "404 Not Found";
                            let resp = format!(
                                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                not_found.len(),
                                not_found
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                        }
                    }
                });
            }
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }
}
