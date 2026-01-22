use serde_json::json;
use tracing::{error, info};

use crate::stats::StatsSnapshot;

/// Generic webhook alerts system
/// Supports Discord, Slack, Telegram, or any webhook-compatible service
#[derive(Clone)]
pub struct WebhookAlerts {
    webhook_url: Option<String>,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl WebhookAlerts {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            webhook_url,
            client: reqwest::Client::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.webhook_url.is_some()
    }

    /// Send an alert via webhook (Discord/Slack/Telegram compatible format)
    pub async fn send_alert(&self, level: AlertLevel, title: &str, message: &str) {
        if !self.is_enabled() {
            return;
        }

        let webhook_url = self.webhook_url.as_ref().unwrap();

        // Choose color based on level
        let color = match level {
            AlertLevel::Info => 0x3498db,      // Blue
            AlertLevel::Warning => 0xf39c12,   // Orange
            AlertLevel::Error => 0xe74c3c,     // Red
            AlertLevel::Critical => 0x992d22,  // Dark Red
        };

        // Choose emoji based on level
        let emoji = match level {
            AlertLevel::Info => "ℹ️",
            AlertLevel::Warning => "⚠️",
            AlertLevel::Error => "❌",
            AlertLevel::Critical => "🚨",
        };

        // Discord/Slack-compatible webhook payload
        let payload = json!({
            "embeds": [{
                "title": format!("{} {}", emoji, title),
                "description": message,
                "color": color,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "footer": {
                    "text": "Synaplan Qdrant Service"
                }
            }]
        });

        match self.client.post(webhook_url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Webhook alert sent: {} - {}", title, message);
                } else {
                    error!(
                        "Failed to send webhook alert: HTTP {}",
                        response.status()
                    );
                }
            }
            Err(e) => {
                error!("Failed to send webhook alert: {}", e);
            }
        }
    }

    /// Alert when service starts
    pub async fn alert_service_started(&self, version: &str) {
        self.send_alert(
            AlertLevel::Info,
            "Service Started",
            &format!("Qdrant microservice v{} is now online", version),
        )
        .await;
    }

    /// Alert when Qdrant connection fails
    pub async fn alert_qdrant_connection_failed(&self, error: &str) {
        self.send_alert(
            AlertLevel::Critical,
            "Qdrant Connection Failed",
            &format!("Cannot connect to Qdrant database: {}", error),
        )
        .await;
    }

    /// Alert when error rate is high
    pub async fn alert_high_error_rate(&self, error_rate: f64, failed: u64, total: u64) {
        // Ensure Warning is used in release builds (avoid dead_code warnings)
        let level = if error_rate >= 20.0 {
            AlertLevel::Error
        } else {
            AlertLevel::Warning
        };

        self.send_alert(
            level,
            "High Error Rate Detected",
            &format!(
                "Error rate is {:.2}% ({} failed out of {} requests)",
                error_rate, failed, total
            ),
        )
        .await;
    }

    /// Send daily statistics report (Discord-optimized format)
    pub async fn send_daily_stats(&self, stats: &StatsSnapshot, collection_name: &str) {
        if !self.is_enabled() {
            return;
        }

        let webhook_url = self.webhook_url.as_ref().unwrap();

        // Discord embed with rich formatting
        let payload = json!({
            "embeds": [{
                "title": "📊 Daily Statistics Report",
                "description": format!("Statistics for collection `{}`", collection_name),
                "color": 0x2ecc71, // Green
                "fields": [
                    {
                        "name": "⬆️ Vectors Upserted",
                        "value": format!("**{}**", format_number(stats.upserts)),
                        "inline": true
                    },
                    {
                        "name": "🔍 Searches Performed",
                        "value": format!("**{}**", format_number(stats.searches)),
                        "inline": true
                    },
                    {
                        "name": "🗑️ Vectors Deleted",
                        "value": format!("**{}**", format_number(stats.deletes)),
                        "inline": true
                    },
                    {
                        "name": "⏱️ Uptime",
                        "value": format!("`{}`", stats.format_uptime()),
                        "inline": false
                    }
                ],
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "footer": {
                    "text": "Synaplan Qdrant Service · Daily Report"
                }
            }]
        });

        match self.client.post(webhook_url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Daily stats webhook sent successfully");
                } else {
                    error!(
                        "Failed to send daily stats webhook: HTTP {}",
                        response.status()
                    );
                }
            }
            Err(e) => {
                error!("Failed to send daily stats webhook: {}", e);
            }
        }
    }
}

/// Format large numbers with thousand separators
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let bytes: Vec<_> = s.bytes().rev().collect();
    let chunks: Vec<_> = bytes
        .chunks(3)
        .map(|chunk| chunk.iter().rev().map(|&b| b as char).collect::<String>())
        .collect();
    chunks.iter().rev().cloned().collect::<Vec<_>>().join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_alerts_disabled() {
        let alerts = WebhookAlerts::new(None);
        assert!(!alerts.is_enabled());
    }

    #[test]
    fn test_webhook_alerts_enabled() {
        let alerts = WebhookAlerts::new(Some("https://example.com/webhook".to_string()));
        assert!(alerts.is_enabled());
    }

    #[test]
    fn test_alert_level_colors() {
        // Just ensure we can match all levels
        let levels = vec![
            AlertLevel::Info,
            AlertLevel::Warning,
            AlertLevel::Error,
            AlertLevel::Critical,
        ];

        for level in levels {
            let _color = match level {
                AlertLevel::Info => 0x3498db,
                AlertLevel::Warning => 0xf39c12,
                AlertLevel::Error => 0xe74c3c,
                AlertLevel::Critical => 0x992d22,
            };
        }
    }

    #[tokio::test]
    async fn test_send_alert_disabled() {
        let alerts = WebhookAlerts::new(None);
        // Should not panic or fail when disabled
        alerts.send_alert(AlertLevel::Info, "Test", "Message").await;
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(1000000000), "1,000,000,000");
    }
}

