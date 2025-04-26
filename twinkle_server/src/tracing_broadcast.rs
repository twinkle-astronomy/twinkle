use tokio::sync::broadcast;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;



// Custom layer for capturing trace events from a specific module
pub struct TracingBroadcast {
    sender: broadcast::Sender<String>,
    target_module: &'static str, // The module to filter on (e.g., "my_app::websocket")
}

impl TracingBroadcast {
    pub fn new(target_module: &'static str, sender: broadcast::Sender<String>) -> Self {
        Self {
            sender,
            target_module,
        }
    }
}


impl<S> Layer<S> for TracingBroadcast
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Check if this event is from our target module
        let target = event.metadata().target();
        
        if target.starts_with(self.target_module) {
            // Format the event data
            // let level = format!("{:?}", event.metadata().level());
            
            // Visit the event fields to extract message and other data
            let mut fields = std::collections::HashMap::new();
            let mut visitor = JsonVisitor(&mut fields);
            event.record(&mut visitor);
            
            // Convert to a structure suitable for the clients
            // let message = serde_json::json!({
            //     "level": level,
            //     "target": target,
            //     "fields": fields,
            //     "timestamp": chrono::Utc::now().to_rfc3339(),
            // }).to_string();
            
            // Broadcast to all connected clients - ignore errors as they just mean
            // there are no receivers connected currently
            if let Some(message) = fields.get("message") {
                let _ = self.sender.send(format!("{} | {}", chrono::Utc::now().to_rfc3339(), message));
            }
        }
    }
}

// Helper to extract fields from a tracing event
struct JsonVisitor<'a>(&'a mut std::collections::HashMap<String, serde_json::Value>);

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(
            field.name().to_string(),
            serde_json::Value::String(format!("{:?}", value)),
        );
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(
            field.name().to_string(),
            serde_json::Value::String(value.to_owned()),
        );
    }
    
    // Implement other record methods as needed for different types
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(
            field.name().to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }
    
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if let Some(value) = serde_json::Number::from_f64(value as f64) {
            self.0.insert(field.name().to_string(), serde_json::Value::Number(value));
        }
    }
    
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(
            field.name().to_string(),
            serde_json::Value::Bool(value),
        );
    }
}
