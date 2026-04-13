/// Configuration Module
/// Handles loading bot settings from .env (credentials) and config.json (behavior)
/// 
/// How it works:
/// 1. .env file stores sensitive data (usernames, passwords) - loaded via dotenvy
/// 2. config.json stores server settings (chest coords, home name, etc.) - loaded via serde_json
/// 3. Supports multiple accounts with "ACTIVE_ACCOUNTS" setting (ranges like 1-5 or 1,3,5)
/// 4. Each tool/material has its own config
/// 5. Account ranges Example: "1-5,7,10-12" = accounts 1,2,3,4,5,7,10,11,12

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// Location of a chest/container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChestLocation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Main configuration struct - holds all bot settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub home_name: String,
    pub obsidian_per_endchest: i32,
    pub shulker_quantity: u32, // Default items per shulker (usually 9 for blocks)
    pub schematic_path: String,
    pub build_origin: (i32, i32, i32),
    
    // Per-tool config (pickaxe, axe, shovel, etc.)
    pub tools: HashMap<String, ToolConfig>,
    
    // Per-material config (obsidian, stone, etc.)
    pub materials: HashMap<String, MaterialConfig>,
}

/// Configuration for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub chest_location: ChestLocation,
}

/// Configuration for a material/block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialConfig {
    pub chest_location: ChestLocation,
    pub is_stackable: bool,
}

/// Account credentials from .env
#[derive(Debug, Clone)]
pub struct Account {
    pub username: String,
    pub password: String,
}

/// Parse account range string into list of indices
/// Examples: "1" -> [1], "1-3" -> [1,2,3], "1,3,5" -> [1,3,5], "1-3,5,7-9" -> [1,2,3,5,7,8,9]
fn parse_account_range(range_str: &str) -> Vec<usize> {
    let mut indices = vec![];
    
    for part in range_str.split(',') {
        let part = part.trim();
        if let Some(dash_idx) = part.find('-') {
            // Range like "1-5"
            if let (Ok(start), Ok(end)) = (
                part[..dash_idx].trim().parse::<usize>(),
                part[dash_idx + 1..].trim().parse::<usize>(),
            ) {
                for i in start..=end {
                    indices.push(i);
                }
            }
        } else if let Ok(idx) = part.parse::<usize>() {
            // Single index like "5"
            indices.push(idx);
        }
    }
    
    // Remove duplicates
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Load config from config.json
pub fn load_bot_config(path: &str) -> anyhow::Result<BotConfig> {
    let content = fs::read_to_string(path)?;
    let config: BotConfig = serde_json::from_str(&content)?;
    
    // Validate essential fields
    if config.home_name.is_empty() {
        anyhow::bail!("config.json: home_name cannot be empty");
    }
    if config.shulker_quantity == 0 {
        anyhow::bail!("config.json: shulker_quantity must be > 0");
    }
    if config.tools.is_empty() {
        anyhow::bail!("config.json: tools section cannot be empty");
    }
    if config.materials.is_empty() {
        anyhow::bail!("config.json: materials section cannot be empty");
    }
    if config.build_origin.0 == 0 && config.build_origin.1 == 0 && config.build_origin.2 == 0 {
        anyhow::bail!("config.json: build_origin must be set (not all zeros)");
    }
    
    Ok(config)
}

/// Load account credentials from .env file
/// Format: USERNAME_1=name PASSWORD_1=pass USERNAME_2=name PASSWORD_2=pass
/// ACTIVE_ACCOUNTS can be: "1" or "1-5" or "1,3,5" or "1-3,5,7-9"
pub fn load_accounts() -> anyhow::Result<Vec<Account>> {
    // Load .env file
    dotenvy::dotenv().ok(); // Ok if file doesn't exist
    
    let mut accounts = vec![];
    
    // Read ACTIVE_ACCOUNTS setting (supports ranges and lists)
    let active_str = std::env::var("ACTIVE_ACCOUNTS").unwrap_or_else(|_| "1".to_string());
    let active_indices = parse_account_range(&active_str);
    
    if active_indices.is_empty() {
        anyhow::bail!("ACTIVE_ACCOUNTS has invalid format: {}", active_str);
    }
    
    // Load each active account
    for idx in active_indices {
        let username_key = format!("USERNAME_{}", idx);
        let password_key = format!("PASSWORD_{}", idx);
        
        let username = match std::env::var(&username_key) {
            Ok(u) => u,
            Err(_) => {
                eprintln!("Warning: {} not found in .env", username_key);
                continue;
            }
        };
        
        let password = std::env::var(&password_key)?;
        
        accounts.push(Account { username, password });
    }
    
    if accounts.is_empty() {
        anyhow::bail!("No active accounts configured in .env (ACTIVE_ACCOUNTS)");
    }
    
    Ok(accounts)
}

/// Load server address from config or .env
pub fn load_server_address() -> String {
    std::env::var("SERVER").unwrap_or_else(|_| "alt.6b6t.org".to_string())
}

/// Load build start setting from .env
pub fn should_start_building() -> bool {
    dotenvy::dotenv().ok();
    std::env::var("START_BUILDING_ON_JOIN")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true"
}

/// Load whitelisted usernames from .env
/// Format: WHITELIST_USERS=Player1,Player2,Player3
pub fn load_whitelist() -> anyhow::Result<Vec<String>> {
    dotenvy::dotenv().ok();
    
    let whitelist_str = std::env::var("WHITELIST_USERS")
        .unwrap_or_else(|_| String::new());
    
    if whitelist_str.is_empty() {
        return Ok(vec![]);
    }
    
    let users: Vec<String> = whitelist_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    Ok(users)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_single_account() {
        assert_eq!(parse_account_range("1"), vec![1]);
        assert_eq!(parse_account_range("5"), vec![5]);
    }
    
    #[test]
    fn test_parse_range() {
        assert_eq!(parse_account_range("1-3"), vec![1, 2, 3]);
        assert_eq!(parse_account_range("5-7"), vec![5, 6, 7]);
    }
    
    #[test]
    fn test_parse_list() {
        assert_eq!(parse_account_range("1,3,5"), vec![1, 3, 5]);
        assert_eq!(parse_account_range("2,1,3"), vec![1, 2, 3]); // Auto-sorted
    }
    
    #[test]
    fn test_parse_mixed() {
        assert_eq!(parse_account_range("1-3,5,7-9").len(), 8);
        assert_eq!(parse_account_range("1-3,5,7-9"), vec![1, 2, 3, 5, 7, 8, 9]);
    }
    
    #[test]
    fn test_parse_deduplication() {
        // If ranges overlap, duplicates removed
        assert_eq!(parse_account_range("1-5,3-7"), vec![1, 2, 3, 4, 5, 6, 7]);
    }
    
    #[test]
    fn test_config_validation() {
        // Config must have tools and materials
        let config = BotConfig {
            home_name: "Test".to_string(),
            obsidian_per_endchest: 8,
            shulker_quantity: 9,
            schematic_path: "test.schematic".to_string(),
            build_origin: (0, 319, 0),
            tools: {
                let mut t = HashMap::new();
                t.insert(
                    "pickaxe".to_string(),
                    ToolConfig {
                        chest_location: ChestLocation { x: 0, y: 100, z: 0 },
                    },
                );
                t
            },
            materials: {
                let mut m = HashMap::new();
                m.insert(
                    "obsidian".to_string(),
                    MaterialConfig {
                        chest_location: ChestLocation { x: 0, y: 100, z: 0 },
                        is_stackable: true,
                    },
                );
                m
            },
        };
        
        assert!(!config.home_name.is_empty());
        assert!(config.shulker_quantity > 0);
        assert!(!config.tools.is_empty());
        assert!(!config.materials.is_empty());
    }
}
