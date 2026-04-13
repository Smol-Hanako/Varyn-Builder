/// Varyn Builder - Minecraft Bot for Schematic Building
/// 
/// Main entry point that loads modules and initializes the bot
/// Modules are organized as:
/// - modules/: Core bot functionality (config, account, inventory, workflow)
/// - plugins/: Extensions (schematic parser)

use azalea::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use azalea::client_chat::ChatPacket;

// ===== MODULE IMPORTS =====
// Each module handles a specific responsibility
mod modules;
mod plugins;

use modules::config;
use modules::account::AccountManager;
use modules::build_workflow::BuildWorkflow;

/// Global bot state shared across event handlers
#[derive(Component, Clone)]
struct BotInstance {
    logged_in: Arc<Mutex<bool>>,
    accounts: Arc<Mutex<AccountManager>>,
    workflow: Arc<Mutex<BuildWorkflow>>,
}

impl Default for BotInstance {
    fn default() -> Self {
        Self {
            logged_in: Arc::new(Mutex::new(false)),
            accounts: Arc::new(Mutex::new(AccountManager::new(vec![]))),
            workflow: Arc::new(Mutex::new(BuildWorkflow::new((0, 0, 0), 8))),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("═══════════════════════════════════════════════════════");
    println!("        Varyn Builder - Minecraft Bot");
    println!("═══════════════════════════════════════════════════════");
    
    // ===== LOAD CONFIGURATION =====
    println!("\n[INIT] Loading configuration...");
    
    // Load account credentials from .env
    let accounts = config::load_accounts()?;
    println!("[INIT] ✓ Loaded {} account(s)", accounts.len());
    
    // Load bot config from config.json
    let bot_config = config::load_bot_config("config.json")?;
    println!("[INIT] ✓ Loaded bot config");
    println!("      Home: {}", bot_config.home_name);
    println!("      Build Y: {}", bot_config.build_y_level);
    
    // Create account manager
    let account_credentials = accounts
        .into_iter()
        .map(|a| (a.username, a.password))
        .collect::<Vec<_>>();
    
    let account_manager = AccountManager::new(account_credentials);
    let current_account = account_manager
        .current()
        .ok_or_else(|| anyhow::anyhow!("No active account"))?
        .clone();
    
    println!("[INIT] ✓ Using account: {}", current_account.username);
    
    // ===== LOAD SCHEMATIC (if path provided) =====
    println!("\n[INIT] Loading schematic...");
    let obsidian_per_chest = bot_config.obsidian_per_endchest as u32;
    let mut workflow = BuildWorkflow::new((0, bot_config.build_y_level as i32, 0), obsidian_per_chest);
    
    if let Ok(schematic) = plugins::schematic::load_schematic(&bot_config.schematic_path) {
        workflow.load_schematic(schematic)?;
        println!("[INIT] ✓ Schematic loaded and verified");
    } else {
        println!("[INIT] ⚠ Schematic not found - will operate in manual mode");
    }
    
    // ===== CONNECT TO SERVER =====
    println!("\n[INIT] Connecting to server...");
    let server = config::load_server_address();
    println!("[INIT] Server: {}", server);
    
    // Note: BotInstance state is created via Default impl by azalea
    // The config is loaded inside handle_event when needed
    ClientBuilder::new()
        .set_handler(handle_event)
        .start(
            Account::offline(&current_account.username),
            server.as_str(),
        )
        .await;
    
    Ok(())
}

/// Main event handler for bot events
async fn handle_event(bot: Client, event: Event, state: BotInstance) -> anyhow::Result<()> {
    match event {
        Event::Login => {
            handle_login(&bot, &state).await?;
        }

        Event::Chat(msg) => {
            handle_chat(&bot, &state, msg).await?;
        }

        Event::Death(_) => {
            handle_death(&bot, &state).await?;
        }

        _ => {}
    }

    Ok(())
}

/// Handle login event - authenticate with server
async fn handle_login(bot: &Client, state: &BotInstance) -> anyhow::Result<()> {
    println!("[EVENT] Login complete. Authenticating in 1 second...");
    
    // Get current account password
    let accounts = state.accounts.lock().unwrap();
    if let Some(account) = accounts.current() {
        let password = account.password.clone();
        drop(accounts); // Release lock
        
        let bot_clone = bot.clone();
        tokio::spawn(async move {
            sleep(Duration::from_secs(1)).await;
            println!("[EVENT] Sending /login command...");
            bot_clone.chat(format!("/login {}", password));
        });
    }
    
    Ok(())
}

/// Handle chat messages - parse responses and trigger actions
async fn handle_chat(bot: &Client, state: &BotInstance, msg: ChatPacket) -> anyhow::Result<()> {
    let content = msg.message().to_string();
    println!("[CHAT] {}", content);

    // Check for successful login
    if content.to_lowercase().contains("logged in") 
        || content.to_lowercase().contains("welcome")
        || content.to_lowercase().contains("successfully") 
    {
        let mut logged_in = state.logged_in.lock().unwrap();
        if !*logged_in {
            *logged_in = true;
            drop(logged_in);
            
            println!("[EVENT] ✓ Logged in successfully! World loading...");

            let bot_clone = bot.clone();
            let workflow = state.workflow.clone();
            
            tokio::spawn(async move {
                sleep(Duration::from_secs(5)).await;

                let pos = bot_clone.position();
                println!(
                    "[BOT] Position: X={:.1}, Y={:.1}, Z={:.1}",
                    pos.x, pos.y, pos.z
                );
                
                // Check if should start building
                if config::should_start_building() {
                    println!("[BOT] START_BUILDING_ON_JOIN enabled - starting build...");
                    
                    let mut wf = workflow.lock().unwrap();
                    if let Err(e) = wf.start() {
                        println!("[ERROR] Failed to start build: {}", e);
                    } else {
                        println!("[BOT] {} - {}", wf.status_report(), wf.inventory.status_string());
                    }
                } else {
                    println!("[BOT] Waiting for build command (set START_BUILDING_ON_JOIN=true to auto-start)");
                }
            });
        }
    }
    
    Ok(())
}

/// Handle death event
async fn handle_death(_bot: &Client, state: &BotInstance) -> anyhow::Result<()> {
    println!("[EVENT] Bot died.");
    
    // Reset inventory on death
    let mut workflow = state.workflow.lock().unwrap();
    workflow.inventory.reset();
    
    println!("[BOT] Inventory cleared. Respawning...");
    
    Ok(())
}