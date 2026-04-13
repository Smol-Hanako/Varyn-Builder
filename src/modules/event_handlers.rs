/// Event Handlers Module
/// Centralized handling for all bot events
/// Only responds to direct messages/whispers to prevent spam on large servers

use azalea::prelude::*;
use azalea::client_chat::ChatPacket;

use super::build_workflow::BuildState;
use super::chat_commands::BotCommand;
use super::login_manager; // ← NEW: import login_manager
use crate::BotInstance;

/// Check if a chat message is a direct whisper to the bot
fn is_whisper(msg_content: &str) -> bool {
    let lower = msg_content.to_lowercase();
    lower.contains("whispers")
}

/// Handle login event — delegates to LoginManager
///
/// CHANGED: was sending /login directly. Now the LoginManager handles
/// the whole flow (send login → wait coord change → walk → wait welcome)
pub async fn handle_login(bot: &Client, state: &BotInstance) -> anyhow::Result<()> {
    println!("[EVENT] Login event — starting login flow");

    // Reset login state on every (re)spawn so the flow always runs fresh
    state.login_manager.lock().unwrap().reset();

    // Hand off to the login manager
    login_manager::on_spawn(bot.clone(), state.login_manager.clone()).await;

    Ok(())
}

/// Handle Tick event — fires ~20 times per second
///
/// NEW: needed so login_manager can watch for coordinate changes
pub async fn handle_tick(bot: &Client, state: &BotInstance) -> anyhow::Result<()> {
    login_manager::on_tick(bot.clone(), state.login_manager.clone()).await;
    Ok(())
}

/// Handle chat messages — parse commands and TPA requests (whispers only)
pub async fn handle_chat(bot: &Client, state: &BotInstance, msg: ChatPacket) -> anyhow::Result<()> {
    let content = msg.message().to_string();
    println!("[CHAT] {}", content);

    // ── NEW: always pass chat to login_manager first ─────────────────────────
    // It needs to see "Welcome" even if it isn't a whisper.
    // This call is cheap — it returns immediately if login is already complete.
    login_manager::on_chat(&content, state.login_manager.clone());
    // ─────────────────────────────────────────────────────────────────────────

    // Only process commands/TPA if login is complete.
    // This stops the bot from reacting to spam while still authenticating.
    if !state.login_manager.lock().unwrap().is_complete() {
        return Ok(());
    }

    // Only process whispers/direct messages (ignore public chat spam)
    if !is_whisper(&content) {
        println!("[CHAT] Ignoring public message (not a whisper)");
        return Ok(());
    }

    // Try to parse as command
    if let Some(command) = super::chat_commands::CommandHandler::parse_command(&content) {
        // Extract sender username — TODO: get from packet metadata
        let sender = "Unknown";

        // Check whitelist for ALL commands
        match command {
            BotCommand::Start
            | BotCommand::Pause
            | BotCommand::Resume
            | BotCommand::Stop
            | BotCommand::TeleportHere(_)
            | BotCommand::Execute(_) => {
                if !state.command_handler.is_whitelisted(sender) {
                    println!("[COMMAND] {} tried forbidden command (NOT WHITELISTED)", sender);
                    bot.chat("❌ Permission denied. You are not whitelisted!");
                    return Ok(());
                }
            }
            _ => {}
        }

        // Whitelist passed — execute commands
        match command {
            BotCommand::Start => {
                println!("[COMMAND] {} issued: $start (WHITELISTED)", sender);
                let mut wf = state.workflow.lock().unwrap();
                if let Err(e) = wf.start() {
                    bot.chat(format!("❌ Error: {}", e));
                } else {
                    bot.chat("✅ Building started!");
                }
            }
            BotCommand::Pause => {
                println!("[COMMAND] {} issued: $pause (WHITELISTED)", sender);
                let mut wf = state.workflow.lock().unwrap();
                wf.state = BuildState::Paused;
                bot.chat("⏸ Building paused!");
            }
            BotCommand::Resume => {
                println!("[COMMAND] {} issued: $resume (WHITELISTED)", sender);
                let mut wf = state.workflow.lock().unwrap();
                wf.state = BuildState::Building;
                bot.chat("▶ Building resumed!");
            }
            BotCommand::Stop => {
                println!("[COMMAND] {} issued: $stop (WHITELISTED)", sender);
                let mut wf = state.workflow.lock().unwrap();
                wf.state = BuildState::Idle;
                bot.chat("⏹ Building stopped!");
            }
            BotCommand::TeleportHere(_) => {
                println!("[COMMAND] {} issued: $tphere (WHITELISTED)", sender);
                bot.chat(format!("📍 Teleporting to {}...", sender));
            }
            BotCommand::Execute(cmd) => {
                println!("[COMMAND] {} issued: $exec {} (WHITELISTED)", sender, cmd);
                bot.chat(&cmd);
            }
            BotCommand::Unknown(cmd) => {
                println!("[CHAT] Unknown command: ${}", cmd);
                bot.chat(format!("⚠ Unknown command: ${}", cmd));
            }
        }
        return Ok(());
    }

    // Check for TPA requests
    if let Some(username) = super::chat_commands::parse_tpa_request(&content) {
        println!("[TPA] Request detected from {}", username);
        state.command_handler.add_tpa_request(username.clone());

        if state.command_handler.is_whitelisted(&username) {
            bot.chat(format!("/tpy {}", username));
            println!("[TPA] ✓ Auto-accepted for whitelisted user: {}", username);
            state.command_handler.clear_tpa_request(&username);
        } else {
            println!("[TPA] ⏳ Pending approval for: {}", username);
        }
    }

    Ok(())
}

/// Handle death event
pub async fn handle_death(_bot: &Client, state: &BotInstance) -> anyhow::Result<()> {
    println!("[EVENT] Bot died.");

    // Reset inventory on death
    let mut workflow = state.workflow.lock().unwrap();
    workflow.inventory.reset();

    println!("[BOT] Inventory cleared. Respawning...");

    Ok(())
}