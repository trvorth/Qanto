// Central error type for qanhash-related operations.
// Keep minimal and Send + Sync for Tokio compatibility.
pub type QanhashError = Box<dyn std::error::Error + Send + Sync>;
