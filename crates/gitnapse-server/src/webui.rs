//! Embedded web UI assets — shipped inside the binary so the API server is
//! self-contained ("packaged and installed complete").

pub const INDEX_HTML: &str = include_str!("webui/index.html");
