/// Chat Commands Module
/// Handles command parsing from chat messages
/// ALL commands require whitelist authorization (spam prevention for large servers)
///
/// Commands (Whitelist Required):
/// - $start           Start building
/// - $pause           Pause building
/// - $resume          Resume building
/// - $stop            Stop building
/// - $tphere          Teleport bot to player
/// - $exec <command>  Execute command as bot like "$exec /tpa" or "$exec /w username hello"
///
/// TPA (Teleport Ask) handling:
/// - Bot listens for TPA requests and accepts/rejects based on whitelist
/// - Accept: /tpy <username>
/// - Reject: /tpn <username>

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Bot command types
#[derive(Debug, Clone, PartialEq)]
pub enum BotCommand {
    /// Start building
    Start,
    /// Pause building
    Pause,
    /// Resume building
    Resume,
    /// Stop building
    Stop,
    /// Teleport bot to sender (sender must be whitelisted)
    TeleportHere(String), // username
    /// Execute a custom command as the bot
    Execute(String), // command to execute
    /// Invalid/unknown command
    Unknown(String),
}

/// Pending TPA request
#[derive(Debug, Clone)]
pub struct TPARequest {
    pub from_username: String,
    pub pending: bool,
}

/// Command handler state
pub struct CommandHandler {
    /// Whitelisted users who can use restricted commands
    whitelist: Arc<Mutex<HashSet<String>>>,
    /// Pending TPA requests (username -> TPARequest)
    pending_tpa: Arc<Mutex<Vec<TPARequest>>>,
}

impl CommandHandler {
    /// Create new command handler
    pub fn new() -> Self {
        Self {
            whitelist: Arc::new(Mutex::new(HashSet::new())),
            pending_tpa: Arc::new(Mutex::new(vec![])),
        }
    }
    
    /// Add user to whitelist
    pub fn add_whitelist(&self, username: String) {
        if let Ok(mut wl) = self.whitelist.lock() {
            wl.insert(username);
        }
    }
    
    /// Remove user from whitelist
    pub fn remove_whitelist(&self, username: &str) {
        if let Ok(mut wl) = self.whitelist.lock() {
            wl.remove(username);
        }
    }
    
    /// Check if user is whitelisted
    pub fn is_whitelisted(&self, username: &str) -> bool {
        self.whitelist
            .lock()
            .map(|wl| wl.contains(username))
            .unwrap_or(false)
    }
    
    /// Parse chat message for commands
    /// Returns Some(command) if chat starts with '$', None otherwise
    pub fn parse_command(message: &str) -> Option<BotCommand> {
        let trimmed = message.trim();
        
        if !trimmed.starts_with('$') {
            return None;
        }
        
        // Remove '$' prefix and split by whitespace
        let cmd_str = &trimmed[1..];
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        
        if parts.is_empty() {
            return Some(BotCommand::Unknown("empty".to_string()));
        }
        
        match parts[0] {
            "start" => Some(BotCommand::Start),
            "pause" => Some(BotCommand::Pause),
            "resume" => Some(BotCommand::Resume),
            "stop" => Some(BotCommand::Stop),
            "tphere" => Some(BotCommand::TeleportHere("unknown".to_string())), // Updated by caller with username
            "exec" => {
                if parts.len() > 1 {
                    let command = parts[1..].join(" ");
                    Some(BotCommand::Execute(command))
                } else {
                    Some(BotCommand::Unknown("exec needs command".to_string()))
                }
            }
            _ => Some(BotCommand::Unknown(parts[0].to_string())),
        }
    }
    
    /// Register a TPA request from a player
    pub fn add_tpa_request(&self, username: String) {
        if let Ok(mut pending) = self.pending_tpa.lock() {
            // Check if already exists
            if !pending.iter().any(|r| r.from_username == username) {
                pending.push(TPARequest {
                    from_username: username,
                    pending: true,
                });
            }
        }
    }
    
    /// Check if there's a pending TPA from a user
    pub fn has_pending_tpa(&self, username: &str) -> bool {
        self.pending_tpa
            .lock()
            .map(|p| p.iter().any(|r| r.from_username == username && r.pending))
            .unwrap_or(false)
    }
    
    /// Accept TPA from a user
    pub fn accept_tpa(&self, username: &str) -> bool {
        if let Ok(mut pending) = self.pending_tpa.lock() {
            if let Some(req) = pending.iter_mut().find(|r| r.from_username == username) {
                req.pending = false;
                return true;
            }
        }
        false
    }
    
    /// Get all pending TPAs
    pub fn get_pending_tpas(&self) -> Vec<String> {
        self.pending_tpa
            .lock()
            .map(|p| {
                p.iter()
                    .filter(|r| r.pending)
                    .map(|r| r.from_username.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Clear all TPA requests (after accepting/rejecting)
    pub fn clear_tpa_request(&self, username: &str) {
        if let Ok(mut pending) = self.pending_tpa.lock() {
            pending.retain(|r| r.from_username != username);
        }
    }
}

impl Default for CommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Check for TPA patterns in chat
/// Returns username if TPA request detected
pub fn parse_tpa_request(message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    
    // Common TPA patterns:
    // "Player1 is requesting to teleport to you!"
    // "TPA request from Player1"
    // "[TPA] Player1 wants to teleport to you"
    
    if lower.contains("teleport") && lower.contains("request") {
        // Try to extract username
        // Look for patterns like "Player1 is requesting" or "from Player1"
        if let Some(start) = message.find("from ") {
            let after_from = &message[start + 5..];
            if let Some(end) = after_from.find(|c: char| !c.is_alphanumeric() && c != '_') {
                let username = &after_from[..end];
                if !username.is_empty() && !username.contains(' ') {
                    return Some(username.to_string());
                }
            }
        }
        
        // Try to find username at start
        if let Some(space) = message.find(' ') {
            let maybe_user = &message[..space];
            if maybe_user.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Some(maybe_user.to_string());
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_commands() {
        assert_eq!(CommandHandler::parse_command("$start"), Some(BotCommand::Start));
        assert_eq!(CommandHandler::parse_command("$pause"), Some(BotCommand::Pause));
        assert_eq!(CommandHandler::parse_command("$resume"), Some(BotCommand::Resume));
        assert_eq!(CommandHandler::parse_command("$stop"), Some(BotCommand::Stop));
        assert_eq!(CommandHandler::parse_command("not a command"), None);
        assert_eq!(CommandHandler::parse_command("hello world"), None);
    }
    
    #[test]
    fn test_parse_exec_command() {
        match CommandHandler::parse_command("$exec /tpa Player1") {
            Some(BotCommand::Execute(cmd)) => assert_eq!(cmd, "/tpa Player1"),
            _ => panic!("Expected Execute command"),
        }
    }
    
    #[test]
    fn test_whitelist() {
        let handler = CommandHandler::new();
        
        assert!(!handler.is_whitelisted("player1"));
        
        handler.add_whitelist("player1".to_string());
        assert!(handler.is_whitelisted("player1"));
        
        handler.remove_whitelist("player1");
        assert!(!handler.is_whitelisted("player1"));
    }
    
    #[test]
    fn test_tpa_requests() {
        let handler = CommandHandler::new();
        
        handler.add_tpa_request("player1".to_string());
        assert!(handler.has_pending_tpa("player1"));
        
        handler.accept_tpa("player1");
        assert!(!handler.has_pending_tpa("player1"));
    }
    
    #[test]
    fn test_parse_tpa_request() {
        let msg1 = "Player1 is requesting to teleport to you!";
        assert_eq!(parse_tpa_request(msg1), Some("Player1".to_string()));
        
        let msg2 = "TPA request from Player2";
        assert_eq!(parse_tpa_request(msg2), Some("Player2".to_string()));
    }
}
