use crate::state::MarketState;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

pub async fn seed_from_rest(state: Arc<RwLock<MarketState>>) {
    // 1. Ticker
    if let Ok(output) = std::process::Command::new("curl")
        .args(["-s", "--max-time", "3", "https://api.bybit.com/v5/market/tickers?category=linear&symbol=BTCUSDT"])
        .output()
    {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(item) = val.get("result").and_then(|r| r.get("list")).and_then(|l| l.get(0)) {
                let mut st = state.write().await;
                st.update_ticker(item);
            }
        }
    }

    // 2. Klines
    if let Ok(output) = std::process::Command::new("curl")
        .args(["-s", "--max-time", "3", "https://api.bybit.com/v5/market/kline?category=linear&symbol=BTCUSDT&interval=1&limit=60"])
        .output()
    {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(list) = val.get("result").and_then(|r| r.get("list")).and_then(|l| l.as_array()) {
                let mut st = state.write().await;
                for item in list {
                    if let Some(arr) = item.as_array() {
                        if arr.len() >= 7 {
                            let start = arr[0].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
                            let open = arr[1].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let high = arr[2].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let low = arr[3].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let close = arr[4].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let volume = arr[5].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let turnover = arr[6].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            st.klines.push(crate::types::Kline {
                                start,
                                open,
                                high,
                                low,
                                close,
                                volume,
                                turnover,
                            });
                        }
                    }
                }
            }
        }
    }

    // 3. Orderbook
    if let Ok(output) = std::process::Command::new("curl")
        .args(["-s", "--max-time", "3", "https://api.bybit.com/v5/market/orderbook?category=linear&symbol=BTCUSDT&limit=50"])
        .output()
    {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(res) = val.get("result") {
                let mut st = state.write().await;
                st.update_orderbook("snapshot", res);
                st.seed_initial_slices(24);
                st.evaluate_anomalies_and_vacuum();
            }
        }
    }

    // 4. Binance Price
    if let Ok(output) = std::process::Command::new("curl")
        .args(["-s", "--max-time", "2", "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT"])
        .output()
    {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(p) = val.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()) {
                let mut st = state.write().await;
                st.binance_price = p;
            }
        }
    }
}

pub async fn run_bybit_ws(state: Arc<RwLock<MarketState>>) {
    let url = "wss://stream.bybit.com/v5/public/linear";
    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                let (mut ws_sink, mut ws_read) = ws_stream.split();

                // Subscribe
                let sub_msg = serde_json::json!({
                    "op": "subscribe",
                    "args": [
                        "orderbook.50.BTCUSDT",
                        "publicTrade.BTCUSDT",
                        "tickers.BTCUSDT",
                        "kline.1.BTCUSDT",
                        "allLiquidation.BTCUSDT"
                    ]
                });
                if ws_sink.send(Message::Text(sub_msg.to_string())).await.is_err() {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                }

                let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(50);

                // Ping sender task
                let tx_ping = tx.clone();
                let ping_handle = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(20));
                    loop {
                        interval.tick().await;
                        let ping = serde_json::json!({"op": "ping"}).to_string();
                        if tx_ping.send(Message::Text(ping)).await.is_err() {
                            break;
                        }
                    }
                });

                // Sink forwarder task
                let sink_handle = tokio::spawn(async move {
                    while let Some(m) = rx.recv().await {
                        if ws_sink.send(m).await.is_err() {
                            break;
                        }
                    }
                });

                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(topic) = val.get("topic").and_then(|t| t.as_str()) {
                                    let mut st = state.write().await;
                                    if topic.starts_with("orderbook") {
                                        let msg_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("delta");
                                        if let Some(data) = val.get("data") {
                                            st.update_orderbook(msg_type, data);
                                        }
                                    } else if topic.starts_with("publicTrade") {
                                        if let Some(data) = val.get("data").and_then(|d| d.as_array()) {
                                            st.update_trades(data);
                                        }
                                    } else if topic.starts_with("tickers") {
                                        if let Some(data) = val.get("data") {
                                            st.update_ticker(data);
                                        }
                                    } else if topic.starts_with("kline") {
                                        if let Some(data) = val.get("data").and_then(|d| d.as_array()).and_then(|a| a.first()) {
                                            st.update_kline(data);
                                        }
                                    } else if topic.starts_with("allLiquidation") || topic.starts_with("liquidation") {
                                        if let Some(data) = val.get("data") {
                                            st.update_liquidation(data);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            let _ = tx.send(Message::Pong(payload)).await;
                        }
                        Ok(Message::Close(_)) | Err(_) => {
                            break;
                        }
                        _ => {}
                    }
                }

                ping_handle.abort();
                sink_handle.abort();
            }
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
}

pub async fn run_binance_ws(state: Arc<RwLock<MarketState>>) {
    let url = "wss://stream.binance.com:9443/ws/btcusdt@ticker";
    loop {
        match connect_async(url).await {
            Ok((mut ws_stream, _)) => {
                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(p) = val.get("c").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()) {
                                    let mut st = state.write().await;
                                    st.binance_price = p;
                                }
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            let _ = ws_stream.send(Message::Pong(payload)).await;
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }
    }
}
