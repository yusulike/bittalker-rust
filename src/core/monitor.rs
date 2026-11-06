//! Binance WebSocket price monitor. Port of `src/core/price_monitor.py`.
//!
//! Runs a background thread hosting a single-threaded tokio runtime that
//! connects to the trade stream, parses the `p` (price) field of each trade
//! message and forwards events over a std channel. Reconnects with a 5s
//! delay, sending a client ping every 60s like the original.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, Copy, Debug)]
pub enum MonitorEvent {
    Price(f64),
    Connected(bool),
}

pub struct PriceMonitor {
    events: Option<std::sync::mpsc::Receiver<MonitorEvent>>,
    stop: Arc<AtomicBool>,
}

impl PriceMonitor {
    /// `symbol` is lowercased like the original (default `btcusdt`).
    pub fn start(symbol: &str, ctx: egui::Context) -> Self {
        let symbol = symbol.to_lowercase();
        let ws_url = format!("wss://stream.binance.com:9443/ws/{symbol}@trade");
        let (tx, rx) = std::sync::mpsc::channel::<MonitorEvent>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);

        std::thread::Builder::new()
            .name("price-monitor".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("WS runtime init error: {e}");
                        return;
                    }
                };
                rt.block_on(run_ws(&ws_url, tx, ctx, &stop_thread));
            })
            .expect("failed to spawn price monitor thread");

        Self {
            events: Some(rx),
            stop,
        }
    }

    /// Take ownership of the event receiver (called once by the UI).
    pub fn take_receiver(&mut self) -> Option<std::sync::mpsc::Receiver<MonitorEvent>> {
        self.events.take()
    }

    /// Signal the background loop to stop. The thread is daemon-like: it
    /// exits on its own and the process may end first (same as the Python
    /// version, which joins with a 1s timeout).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

async fn run_ws(url: &str, tx: Sender<MonitorEvent>, ctx: egui::Context, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        let _ = tx.send(MonitorEvent::Connected(false));
        ctx.request_repaint();
        println!("Connecting to {url}...");

        match tokio_tungstenite::connect_async(url).await {
            Ok((ws, _resp)) => {
                println!("WebSocket Connected");
                let _ = tx.send(MonitorEvent::Connected(true));
                ctx.request_repaint();

                let (mut write, mut read) = ws.split();
                let mut ping_interval =
                    tokio::time::interval(Duration::from_secs(60));
                ping_interval.tick().await; // first tick completes immediately
                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    if let Some(price) = parse_price(&text) {
                                        let _ = tx.send(MonitorEvent::Price(price));
                                        ctx.request_repaint();
                                    }
                                }
                                Some(Ok(Message::Ping(payload))) => {
                                    // tungstenite auto-queues pongs, but the
                                    // original relies on the library too
                                    let _ = write.send(Message::Pong(payload)).await;
                                }
                                Some(Ok(Message::Close(None))) | None => break,
                                Some(Ok(_)) => {}
                                Some(Err(e)) => {
                                    eprintln!("WebSocket Error: {e}");
                                    break;
                                }
                            }
                        }
                        _ = ping_interval.tick() => {
                            if write.send(Message::Ping(Vec::new().into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    if stop.load(Ordering::Relaxed) {
                        let _ = write.send(Message::Close(None)).await;
                        return;
                    }
                }
                println!("WebSocket Closed");
                let _ = tx.send(MonitorEvent::Connected(false));
                ctx.request_repaint();
            }
            Err(e) => {
                eprintln!("WebSocket Error: {e}");
            }
        }

        if stop.load(Ordering::Relaxed) {
            return;
        }
        println!("WS Reconnecting in 5s...");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Parse a trade message; returns the `p` (price) field when present.
fn parse_price(text: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let p = v.get("p")?;
    p.as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| p.as_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_trade_price() {
        let msg = r#"{"e":"trade","p":"95432.12","q":"0.001","T":1737000000000}"#;
        assert_eq!(parse_price(msg), Some(95432.12));
    }

    #[test]
    fn ignores_non_trade_messages() {
        assert_eq!(parse_price(r#"{"e":"pong"}"#), None);
        assert_eq!(parse_price("not json"), None);
    }
}
