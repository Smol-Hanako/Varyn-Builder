/// Build Workflow Module
/// Implements the main building loop according to the schematic
/// This orchestrates the entire bot behavior:
///
/// [START]
///    ↓
/// [CHECK INVENTORY] ← ────────────────────────┐
///    ↓ has materials                           │
/// [TELEPORT → /home]                           │
///    ↓                                         │
/// [BUILD LOOP]                                 │
///  - check block at pos                        │
///  - if wrong → break with tool                │
///  - if air/correct → place                    ↓
///  - if low inventory → /kill → [REFILL] ──→→
///    ↓
/// [DONE]

use crate::modules::inventory::BotInventory;
use crate::plugins::schematic::Schematic;
use std::time::{Duration, Instant};

/// Represents the current build state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildState {
    Idle,
    CheckingInventory,
    Teleporting,
    Building,
    Refilling,
    Paused,
    Completed,
    Failed,
}

impl std::fmt::Display for BuildState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::CheckingInventory => write!(f, "Checking Inventory"),
            Self::Teleporting => write!(f, "Teleporting to /home"),
            Self::Building => write!(f, "Building"),
            Self::Refilling => write!(f, "Refilling materials"),
            Self::Paused => write!(f, "Paused"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// Tracks a single block placement/breaking action
#[derive(Debug, Clone)]
pub struct BlockAction {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub action: BlockActionType,
    pub blocks_queued: usize,
}

#[derive(Debug, Clone)]
pub enum BlockActionType {
    Place(String), // Place block type
    Break,         // Break block (mined by tool)
}

/// Main workflow controller
pub struct BuildWorkflow {
    pub state: BuildState,
    pub schematic: Option<Schematic>,
    pub inventory: BotInventory,
    pub origin: (i32, i32, i32), // Anchor point where schematic starts
    
    // Tracking
    pub blocks_placed: usize,
    pub blocks_broken: usize,
    pub blocks_total: usize,
    pub started_at: Option<Instant>,
    pub last_teleport: Option<Instant>,
    pub teleport_cooldown: Duration,
}

impl BuildWorkflow {
    /// Create new workflow
    pub fn new(origin: (i32, i32, i32), obsidian_per_chest: u32) -> Self {
        Self {
            state: BuildState::Idle,
            schematic: None,
            inventory: BotInventory::new(obsidian_per_chest),
            origin,
            blocks_placed: 0,
            blocks_broken: 0,
            blocks_total: 0,
            started_at: None,
            last_teleport: None,
            teleport_cooldown: Duration::from_secs(5), // Adjust based on server /home cooldown
        }
    }
    
    /// Load schematic for building
    pub fn load_schematic(&mut self, schematic: Schematic) -> anyhow::Result<()> {
        // Verify schematic is valid
        let verification = schematic.verify()?;
        
        println!("[BUILD] Schematic loaded:");
        println!("  Dimensions: {}x{}x{}", schematic.width, schematic.height, schematic.length);
        println!("  Total blocks: {}", verification.total_blocks);
        println!("  Unique types: {}", verification.unique_block_types);
        
        self.blocks_total = verification.total_blocks;
        self.schematic = Some(schematic);
        
        Ok(())
    }
    
    /// Start the build process
    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.schematic.is_none() {
            anyhow::bail!("No schematic loaded");
        }
        
        self.state = BuildState::CheckingInventory;
        self.started_at = Some(Instant::now());
        
        println!("[BUILD] Started! Origin: {:?}", self.origin);
        
        Ok(())
    }
    
    /// Check inventory, return if sufficient materials available
    pub fn check_inventory_sufficient(&self) -> bool {
        // For now, check if at least one slot isn't empty
        // In production: scan schematic and verify all materials available
        !self.inventory.is_low()
    }
    
    /// Transition to teleporting state
    pub fn request_teleport(&mut self) -> bool {
        let now = Instant::now();
        
        // Check cooldown
        if let Some(last) = self.last_teleport {
            if now.duration_since(last) < self.teleport_cooldown {
                return false; // Cooldown active
            }
        }
        
        self.last_teleport = Some(now);
        self.state = BuildState::Teleporting;
        true
    }
    
    /// Move to building state (called after teleport completes)
    pub fn teleport_completed(&mut self) {
        self.state = BuildState::Building;
        println!("[BUILD] Arrived at /home. Starting build...");
    }
    
    /// Generate next block action from schematic
    pub fn next_block_action(&self) -> Option<BlockAction> {
        let schematic = self.schematic.as_ref()?;
        
        // Simplified: just get first unbuilt block
        if self.blocks_placed < schematic.blocks.len() {
            let block = &schematic.blocks[self.blocks_placed];
            
            // Convert schematic coords to world coords
            let world_x = self.origin.0 + block.x as i32;
            let world_y = self.origin.1 + block.y as i32;
            let world_z = self.origin.2 + block.z as i32;
            
            return Some(BlockAction {
                x: world_x,
                y: world_y,
                z: world_z,
                action: BlockActionType::Place(block.name.clone()),
                blocks_queued: schematic.blocks.len() - self.blocks_placed,
            });
        }
        
        None
    }
    
    /// Record block placement success
    pub fn block_placed(&mut self) {
        self.blocks_placed += 1;
    }
    
    /// Record block break success
    pub fn block_broken(&mut self) {
        self.blocks_broken += 1;
    }
    
    /// Check if materials are low (trigger refill cycle)
    pub fn should_refill(&self) -> bool {
        self.inventory.is_low()
    }
    
    /// Transition to refill mode
    pub fn enter_refill_mode(&mut self) {
        self.state = BuildState::Refilling;
        println!("[BUILD] Inventory low! Refilling materials...");
    }
    
    /// Build complete
    pub fn complete(&mut self) {
        self.state = BuildState::Completed;
        
        if let Some(started) = self.started_at {
            let duration = started.elapsed();
            println!(
                "[BUILD] ✓ Complete! Placed: {}, Broken: {}, Time: {:?}",
                self.blocks_placed, self.blocks_broken, duration
            );
        }
    }
    
    /// Build failed/paused
    pub fn fail(&mut self, reason: &str) {
        self.state = BuildState::Failed;
        println!("[BUILD] ✗ Failed: {}", reason);
    }
    
    /// Progress percentage (0-100)
    pub fn progress_percent(&self) -> u32 {
        if self.blocks_total == 0 {
            return 0;
        }
        ((self.blocks_placed as f32 / self.blocks_total as f32) * 100.0) as u32
    }
    
    /// Get status report
    pub fn status_report(&self) -> String {
        format!(
            "[WORKFLOW] State: {} | Progress: {}/{} ({})% | Inventory: {}%",
            self.state,
            self.blocks_placed,
            self.blocks_total,
            self.progress_percent(),
            self.inventory.fullness_percentage()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_workflow_creation() {
        let workflow = BuildWorkflow::new((0, 319, 0), 8);
        assert_eq!(workflow.state, BuildState::Idle);
        assert_eq!(workflow.blocks_placed, 0);
    }
    
    #[test]
    fn test_teleport_cooldown() {
        let mut workflow = BuildWorkflow::new((0, 319, 0), 8);
        
        // First teleport should succeed
        assert!(workflow.request_teleport());
        
        // Immediate second should fail (cooldown)
        assert!(!workflow.request_teleport());
    }
    
    #[test]
    fn test_progress_calculation() {
        let mut workflow = BuildWorkflow::new((0, 319, 0), 8);
        workflow.blocks_total = 100;
        workflow.blocks_placed = 50;
        
        assert_eq!(workflow.progress_percent(), 50);
    }
}
