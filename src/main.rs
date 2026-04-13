/// Varyn Builder - Minecraft Bot for Schematic Building
///
/// Lean entry point: Load config → Initialize state → Connect to server
/// All event handling delegated to modules

use azalea::prelude::*;
use std::sync::{Arc, Mutex};

mod modules;
mod plugins;

use modules::config;
use modules::account::AccountManager;
use modules::build_workflow::BuildWorkflow;
use modules::chat_commands::CommandHandler;
use modules::login_manager::LoginManager; // ← NEW
use modules::event_handlers;

/// Global bot state shared across event handlers
#[derive(Component, Clone)]
struct BotInstance {
    logged_in:       Arc<Mutex<bool>>,
    accounts:        Arc<Mutex<AccountManager>>,
    workflow:        Arc<Mutex<BuildWorkflow>>,
    command_handler: Arc<CommandHandler>,
    login_manager:   Arc<Mutex<LoginManager>>, // ← NEW
}

impl Default for BotInstance {
    fn default() -> Self {
        Self {
            logged_in:       Arc::new(Mutex::new(false)),
            accounts:        Arc::new(Mutex::new(AccountManager::new(vec![]))),
            command_handler: Arc::new(CommandHandler::new()),
            workflow:        Arc::new(Mutex::new(BuildWorkflow::new((0, 0, 0), 8))),
            // Empty password in Default — replaced in main() with the real one
            login_manager:   Arc::new(Mutex::new(LoginManager::new(String::new()))), // ← NEW
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("═══════════════════════════════════════════════════════");
    println!("        Varyn Builder - Minecraft Bot");
    println!("═══════════════════════════════════════════════════════\n");

    println!("[INIT] Loading configuration...");
    let accounts   = config::load_accounts()?;
    let bot_config = config::load_bot_config("config.json")?;
    let whitelist  = config::load_whitelist()?;

    println!("[INIT] ✓ Loaded {} account(s)", accounts.len());
    println!("[INIT] ✓ Bot config - Home: {}, Origin: {:?}",
        bot_config.home_name, bot_config.build_origin);

    // Load schematic
    println!("[INIT] Loading schematic...");
    let obsidian_per_chest = bot_config.obsidian_per_endchest as u32;
    let build_origin       = bot_config.build_origin;
    let mut workflow       = BuildWorkflow::new(build_origin, obsidian_per_chest);

    if let Ok(schematic) = plugins::schematic::load_schematic(&bot_config.schematic_path) {
        workflow.load_schematic(schematic)?;
        println!("[INIT] ✓ Schematic loaded");
    } else {
        println!("[INIT] ⚠ Schematic not found - manual mode");
    }

    // Initialize accounts
    let account_manager = AccountManager::new(
        accounts.into_iter().map(|a| (a.username, a.password)).collect()
    );
    let current = account_manager
        .current()
        .ok_or_else(|| anyhow::anyhow!("No accounts"))?
        .clone();

    println!("[INIT] ✓ Using account: {}", current.username);

    // ── NEW: grab the password for the login manager ─────────────────────────
    let password = current.password.clone();
    // ─────────────────────────────────────────────────────────────────────────

    // Setup command handler with whitelist
    let handler = CommandHandler::new();
    for user in &whitelist {
        handler.add_whitelist(user.clone());
    }
    println!("[INIT] ✓ Whitelist loaded: {} users\n", whitelist.len());

    // Create bot state
    let bot_state = BotInstance {
        logged_in:       Arc::new(Mutex::new(false)),
        accounts:        Arc::new(Mutex::new(account_manager)),
        workflow:        Arc::new(Mutex::new(workflow)),
        command_handler: Arc::new(handler),
        login_manager:   Arc::new(Mutex::new(LoginManager::new(password))), // ← NEW
    };

    // Connect to server
println!("[INIT] Connecting to server...");
let server = config::load_server_address();
println!("[INIT] Server: {}\n", server);

    ClientBuilder::new()
        .set_handler(handle_event)
        .set_state(bot_state)
        .start(Account::offline(&current.username), server.as_str())
        .await;

    Ok(())
}

/// Event dispatcher — routes to handlers in modules
async fn handle_event(bot: Client, event: Event, state: BotInstance) -> anyhow::Result<()> {
    match event {
        Event::Login     => event_handlers::handle_login(&bot, &state).await?,
        Event::Tick      => event_handlers::handle_tick(&bot, &state).await?,  // ← NEW
        Event::Chat(msg) => event_handlers::handle_chat(&bot, &state, msg).await?,
        Event::Death(_)  => event_handlers::handle_death(&bot, &state).await?,
        _ => {}
    }
    Ok(())
}