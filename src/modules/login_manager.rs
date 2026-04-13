/// Login Manager
/// Handles the full authentication flow for cracked/auth servers like 6b6t
///
/// STATE MACHINE FLOW:
///
///  [Spawned]
///     │
///     ▼
///  [SentLogin]  ←── sends "/login <password>" after 1 second delay
///     │
///     ▼
///  [WaitCoordChange]  ←── server teleports us after auth, so coords shift
///     │  (checked every game Tick)
///     ▼
///  [WalkingForward]  ←── walk for 3 seconds (some servers require movement)
///     │
///     ▼
///  [WaitingForWelcome]  ←── listen in chat for "Welcome" or "logged in"
///     │
///     ▼
///  [Complete]  ✓  Bot is fully authenticated!
///
///  At any step → [Failed("reason")]  if something goes wrong

use azalea::prelude::*;
use azalea::WalkDirection;
use tokio::time::{sleep, Duration};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
//  STATE ENUM
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LoginState {
    NotStarted,
    SentLogin,
    WaitCoordChange {
        initial_x: f64,
        initial_z: f64,
    },
    WalkingForward,
    WaitingForWelcome,
    Complete,
    Failed(String),
}

// ─────────────────────────────────────────────────────────────────────────────
//  LOGIN MANAGER STRUCT
// ─────────────────────────────────────────────────────────────────────────────

pub struct LoginManager {
    pub state: LoginState,
    pub password: String,
    tick_counter: u32,
}

impl LoginManager {
    pub fn new(password: String) -> Self {
        Self {
            state: LoginState::NotStarted,
            password,
            tick_counter: 0,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.state == LoginState::Complete
    }

    pub fn has_failed(&self) -> bool {
        matches!(self.state, LoginState::Failed(_))
    }

    pub fn reset(&mut self) {
        println!("[LOGIN] Resetting login state for reconnect");
        self.state = LoginState::NotStarted;
        self.tick_counter = 0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  EVENT HANDLERS
// ─────────────────────────────────────────────────────────────────────────────

pub async fn on_spawn(bot: Client, login_manager: Arc<Mutex<LoginManager>>) {
    println!("[LOGIN] Spawn event — scheduling /login in 1 second");

    let bot_clone  = bot.clone();
    let lm_clone   = login_manager.clone();

    tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;

        let mut password = {
            let lm = lm_clone.lock().unwrap();
            lm.password.clone()
        }; // ← mutex released automatically here

        // ── FIX: Emergency Fallback for Empty Passwords ──────────────────────
        // If BotInstance was injected via Bevy's Default trait instead
        // of your initialized instance, this string will be empty.
        if password.trim().is_empty() {
            println!("[LOGIN] ⚠ WARNING: Password state is empty! Attempting fallback to config...");
            if let Ok(accounts) = crate::modules::config::load_accounts() {
                if let Some(first_acc) = accounts.first() {
                    password = first_acc.password.clone();
                    println!("[LOGIN] ✓ Recovered password from config fallback.");
                }
            }
        }
        // ─────────────────────────────────────────────────────────────────────

        if password.trim().is_empty() {
            println!("[LOGIN] ❌ FATAL: Password is STILL empty. Sending blank login command will fail.");
        } else {
            println!("[LOGIN] Sending /login ****");
        }

        bot_clone.chat(&format!("/login {}", password));

        let pos = bot_clone.position();

        {
            let mut lm = lm_clone.lock().unwrap();
            lm.state = LoginState::WaitCoordChange {
                initial_x: pos.x,
                initial_z: pos.z,
            };
            lm.tick_counter = 0;
        }

        println!("[LOGIN] State → WaitCoordChange  (standing at {:.1}, {:.1})", pos.x, pos.z);
    });
}

pub async fn on_tick(bot: Client, login_manager: Arc<Mutex<LoginManager>>) {
    let state_snapshot = {
        let lm = login_manager.lock().unwrap();
        lm.state.clone()
    };

    let (initial_x, initial_z) = match state_snapshot {
        LoginState::WaitCoordChange { initial_x, initial_z } => (initial_x, initial_z),
        _ => return, 
    };

    let pos = bot.position();
    let moved = (pos.x - initial_x).abs() > 0.5 || (pos.z - initial_z).abs() > 0.5;

    {
        let mut lm = login_manager.lock().unwrap();
        lm.tick_counter += 1;

        if lm.tick_counter > 200 && !moved {
            lm.state = LoginState::Failed(
                "Timed out: server never moved us after /login (wrong password?)".to_string(),
            );
            println!("[LOGIN] ✗ Timeout waiting for coord change");
            return;
        }
    }

    if !moved {
        return; 
    }

    println!(
        "[LOGIN] ✓ Coord change detected ({:.1},{:.1} → {:.1},{:.1})",
        initial_x, initial_z, pos.x, pos.z
    );

    {
        let mut lm = login_manager.lock().unwrap();
        lm.state = LoginState::WalkingForward;
    }

    let bot_clone = bot.clone();
    let lm_clone  = login_manager.clone();

    tokio::spawn(async move {
        println!("[LOGIN] Walking forward for 3 seconds...");
        bot_clone.walk(WalkDirection::Forward);

        sleep(Duration::from_secs(3)).await;

        bot_clone.walk(WalkDirection::None);
        println!("[LOGIN] Stopped — now waiting for Welcome message");

        let mut lm = lm_clone.lock().unwrap();
        lm.state = LoginState::WaitingForWelcome;
    });
}

pub fn on_chat(message: &str, login_manager: Arc<Mutex<LoginManager>>) {
    let lower = message.to_lowercase();

    let state_snapshot = {
        let lm = login_manager.lock().unwrap();
        lm.state.clone()
    };

    match state_snapshot {
        LoginState::WaitingForWelcome => {
            let is_welcome = lower.contains("welcome")
                || lower.contains("logged in")
                || lower.contains("successfully logged")
                || lower.contains("you are now logged");

            if is_welcome {
                let mut lm = login_manager.lock().unwrap();
                lm.state = LoginState::Complete;
                println!("[LOGIN] ✓ Welcome detected — bot is FULLY LOGGED IN");
            }
        }

        LoginState::SentLogin | LoginState::WaitCoordChange { .. } => {
            if lower.contains("wrong password")
                || lower.contains("incorrect password")
                || lower.contains("invalid password")
            {
                let mut lm = login_manager.lock().unwrap();
                lm.state = LoginState::Failed("Server rejected the password".to_string());
                println!("[LOGIN] ✗ Server rejected the password!");
            }
        }

        _ => {} 
    }
}