use serde_json::json;
use tracing::{error, info};

#[derive(Clone)]
pub struct DiscordAlerts {
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

impl DiscordAlerts {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            webhook_url,
            client: reqwest::Client::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.webhook_url.is_some()
    }

    /// Send an alert to Discord
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

        // Add @here for critical alerts
        let content = if level == AlertLevel::Critical {
            Some("@here **CRITICAL ALERT**".to_string())
        } else {
            None
        };

        let payload = json!({
            "content": content,
            "embeds": [{
                "title": format!("{} {}", emoji, title),
                "description": message,
                "color": color,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "footer": {
                    "text": "Synaplan Qdrant Microservice"
                }
            }]
        });

        match self.client.post(webhook_url).json(&payload).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Discord alert sent: {} - {}", title, message);
                } else {
                    error!(
                        "Failed to send Discord alert: HTTP {}",
                        response.status()
                    );
                }
            }
            Err(e) => {
                error!("Failed to send Discord alert: {}", e);
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

    /// Alert when service is shutting down
    pub async fn alert_service_stopping(&self) {
        self.send_alert(
            AlertLevel::Warning,
            "Service Stopping",
            "Qdrant microservice is shutting down",
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
        self.send_alert(
            AlertLevel::Error,
            "High Error Rate Detected",
            &format!(
                "Error rate is {:.2}% ({} failed out of {} requests)",
                error_rate, failed, total
            ),
        )
        .await;
    }

    /// Alert when collection is getting full
    pub async fn alert_collection_high_usage(&self, points: u64) {
        self.send_alert(
            AlertLevel::Warning,
            "High Collection Usage",
            &format!("Collection has {} points. Consider monitoring growth.", points),
        )
        .await;
    }

    /// Alert on panic or critical error
    pub async fn alert_panic(&self, error: &str) {
        self.send_alert(
            AlertLevel::Critical,
            "Service Panic",
            &format!("PANIC: {}", error),
        )
        .await;
    }
}

