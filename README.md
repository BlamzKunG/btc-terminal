# btc-terminal

High-performance real-time Bitcoin (BTC/USDT) microstructure and order flow terminal for the command line, built in Rust with Ratatui and Tokio.

The terminal streams low-latency market data directly from Bybit and Binance WebSockets, providing deep orderbook visualization, BookMap-style depth matrices, trade intensity, Cumulative Volume Delta (CVD), liquidity vacuum detection, and multi-exchange arbitrage monitoring.

## Key Features

- Low-Latency Rust Engine: Built using Tokio asynchronous runtime, Ratatui terminal UI, and non-blocking WebSockets.
- Dual-Exchange Data Feeds:
  - Bybit Linear Futures: 50-level real-time orderbook, tick-by-tick public trade stream, 1-minute klines, tickers, and liquidations.
  - Binance Futures: Sub-second ticker stream for live cross-exchange basis and arbitrage monitoring.
- BookMap Depth Matrix and Heatmap Ladder:
  - Configurable aggregation steps ($1, $2, $5, $10, etc.).
  - Dynamic depth rows (5, 8, 10, 15, 20 levels or terminal auto-fit).
  - Relative heatmap coloring for bids (Aqua/Green) and asks (Gold/Red).
  - Real-time resting liquidity wall identification.
- 2D Time-Series BookMap Heatmap:
  - Historical resting liquidity and depth evolution across price tiers over time.
  - Live price trajectory overlay with trade marker identification.
- Order Flow and Microstructure Analytics:
  - Session Cumulative Volume Delta (CVD) tracking aggressive buyers versus aggressive sellers.
  - Real-time Bid/Ask pressure gauge and depth ratio.
  - Liquidity Vacuum detection alerting to dangerously thin liquidity ahead of price.
  - Flow Anomaly Detector identifying sudden volume spikes and spread expansions.
  - Whale and Large Trade Radar with configurable BTC threshold.
- 4-Layer Confluence Signal Engine:
  - Multi-factor scoring combining trend, momentum, orderbook pressure, and vacuum conditions.
  - Technical indicator engine calculating live 14-period RSI, 9-period EMA, and 21-period EMA on 1-minute intervals.
- Terminal Control and Customization:
  - Clean terminal restore on exit.
  - Persistent settings stored in standard configuration directory.
  - Built-in embedded real-time Web Dashboard with interactive Candlestick Chart (OHLCV).
  - Web UI can be toggled on/off dynamically from the CLI settings menu (`m`).

## Terminal Views & Web Interface

1. Dashboard View (Tab 1): Comprehensive overview featuring price, 24h range, 25-minute sparkline trend, multi-exchange spread, confluence signals, technicals, whale radar, liquidity vacuum, and microstructure pressure.
2. Ladder / Depth Matrix View (Tab 2): Vertical orderbook ladder showing live resting bid and ask depth, dynamic heatmap bars, liquidity walls, mid-price boundary, and session CVD.
3. 2D Heatmap View (Tab 3): Matrix displaying orderbook depth evolution across price bands over time slices.
4. Web Dashboard (`http://localhost:8080`): Browser-based live Candlestick (OHLCV) chart with EMA 9/21, Volume histogram, real-time Orderbook pressure bar, CVD delta, and live Trade Tape. Toggleable via CLI menu (`m`).

## Keyboard Shortcuts

- Tab / 1 / 2 / 3: Switch between Dashboard, Ladder, and 2D Heatmap views.
- r / R: Cycle depth rows on BookMap and Ladder (5, 8, 10, 15, 20 rows).
- [ / ]: Decrease or increase depth rows step-by-step.
- + / =: Increase price aggregation step.
- - / _: Decrease price aggregation step.
- w / W: Cycle whale trade threshold (0.5, 1.0, 2.0, 5.0, 10.0 BTC).
- m: Open / close terminal display configuration menu (toggle Web Dashboard, Whale Radar, Signals, etc.).
- q: Exit cleanly and restore terminal buffer.

## Installation and Build

### Prerequisites

- Rust and Cargo (edition 2021 or newer)
- OpenSSL / development libraries

### Building from Source

```bash
git clone https://github.com/BlamzKunG/btc-terminal.git
cd btc-terminal
cargo build --release
```

The compiled binary will be located at `target/release/btc_rs`.

### System Installation

To install system-wide as `btc`:

```bash
cp target/release/btc_rs /usr/local/bin/btc
chmod +x /usr/local/bin/btc
```

## Command-Line Arguments

```text
Usage: btc [OPTIONS]

Options:
  --dash           Start directly in Dashboard view (Page 1)
  --book           Start directly in BookMap / Ladder view (Page 2)
  --heat           Start directly in 2D Heatmap view (Page 3)
  --web            Start in headless Web Server mode (http://localhost:8080)
  -p, --page <N>   Select starting page (1, 2, or 3)
  -i <SECONDS>     Set UI refresh interval in seconds (default: 0.1)
  --once           Render a single frame snapshot and exit
  --menu           Open settings menu on launch
  -h, --help       Print help information
```

Examples:

```bash
# Run with default settings
btc

# Open directly to BookMap ladder with 20ms refresh rate
btc --book -i 0.02

# Print a single terminal snapshot
btc --once
```

## Configuration

Settings are saved automatically to:

- Linux / Unix: `~/.config/btc/settings.json`

Example configuration:

```json
{
  "default_page": 2,
  "show_signal": true,
  "show_technical": true,
  "show_whale_radar": true,
  "show_vacuum_radar": true,
  "show_microstructure": true,
  "show_derivatives": true,
  "show_candles": true,
  "ladder_step": 1.0,
  "whale_threshold": 2.0,
  "ladder_rows": 10
}
```

## Architecture

- `src/main.rs`: Application entry point, CLI argument parsing, event polling loop, and panic restoration hooks.
- `src/ws.rs`: Asynchronous WebSocket clients for Bybit Linear and Binance Futures with automatic reconnection and heartbeat pinging.
- `src/state.rs`: Thread-safe market state management, orderbook bucket aggregation, technical indicators, confluence calculation, and anomaly detection.
- `src/ui.rs`: Terminal rendering engine powered by Ratatui with custom cell layout, dynamic sizing, and semantic color formatting.
- `src/types.rs`: Core data structures, serialization schemas, and settings persistence.

## License

MIT License
