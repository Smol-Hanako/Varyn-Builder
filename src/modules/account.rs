/// Account Module
/// Manages multiple accounts and switching between them
/// 
/// Each account has login state tracking and can be cycled
/// Useful for load balancing or account rotation

use std::sync::{Arc, Mutex};

/// Tracks a single account's state
#[derive(Debug, Clone)]
pub struct AccountState {
    pub username: String,
    pub password: String,
    pub is_logged_in: Arc<Mutex<bool>>,
    pub account_id: usize,
}

impl AccountState {
    pub fn new(username: String, password: String, account_id: usize) -> Self {
        Self {
            username,
            password,
            is_logged_in: Arc::new(Mutex::new(false)),
            account_id,
        }
    }
    
    /// Mark account as logged in
    pub fn mark_logged_in(&self) {
        if let Ok(mut state) = self.is_logged_in.lock() {
            *state = true;
        }
    }
    
    /// Mark account as logged out
    pub fn mark_logged_out(&self) {
        if let Ok(mut state) = self.is_logged_in.lock() {
            *state = false;
        }
    }
    
    /// Check if account is logged in
    pub fn check_logged_in(&self) -> bool {
        self.is_logged_in.lock().map(|s| *s).unwrap_or(false)
    }
}

/// Account manager for multi-account support
pub struct AccountManager {
    accounts: Vec<AccountState>,
    current_index: usize,
}

impl AccountManager {
    /// Create new account manager from credentials
    pub fn new(accounts: Vec<(String, String)>) -> Self {
        let account_states = accounts
            .into_iter()
            .enumerate()
            .map(|(idx, (username, password))| {
                AccountState::new(username, password, idx)
            })
            .collect();
        
        Self {
            accounts: account_states,
            current_index: 0,
        }
    }
    
    /// Get current active account
    pub fn current(&self) -> Option<&AccountState> {
        self.accounts.get(self.current_index)
    }
    
    /// Switch to next account (for rotation)
    pub fn next_account(&mut self) {
        self.current_index = (self.current_index + 1) % self.accounts.len();
    }
    
    /// Get account by ID
    pub fn get_account(&self, id: usize) -> Option<&AccountState> {
        self.accounts.iter().find(|a| a.account_id == id)
    }
    
    /// Get all accounts
    pub fn all_accounts(&self) -> &[AccountState] {
        &self.accounts
    }
    
    /// Count logged in accounts
    pub fn logged_in_count(&self) -> usize {
        self.accounts
            .iter()
            .filter(|a| a.check_logged_in())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_account_state() {
        let account = AccountState::new("player".to_string(), "pass".to_string(), 0);
        assert!(!account.check_logged_in());
        
        account.mark_logged_in();
        assert!(account.check_logged_in());
    }
    
    #[test]
    fn test_account_manager() {
        let mut manager = AccountManager::new(vec![
            ("player1".to_string(), "pass1".to_string()),
            ("player2".to_string(), "pass2".to_string()),
        ]);
        
        assert_eq!(manager.current().unwrap().username, "player1");
        manager.next_account();
        assert_eq!(manager.current().unwrap().username, "player2");
    }
}
