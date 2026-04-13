// Modules - Organizational core of the bot
// Each module handles a specific responsibility

pub mod config;    // Load .env and config.json settings
pub mod account;   // Multi-account management
pub mod inventory; // Track materials and shulker logic
pub mod build_workflow; // Main building loop
