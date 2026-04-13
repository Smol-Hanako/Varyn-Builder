/// Inventory Module
/// Manages shulker boxes and material tracking
/// 
/// Key concepts:
/// - 9 slots (0-8) for shulker boxes containing blocks
/// - Obsidian is special: can be in shulkers OR in ender chests (1 EC = 8 obsidian)
/// - Tracks low inventory state for refill logic

use std::collections::HashMap;

/// Represents a shulker box slot
#[derive(Debug, Clone)]
pub struct ShulkerSlot {
    pub slot_index: u8, // 0-8
    pub material: String, // e.g., "obsidian", "stone"
    pub stack_count: u32, // How many items in this shulker
    pub max_stack: u32, // Usually 64 or 1 for certain blocks
}

impl ShulkerSlot {
    pub fn new(slot_index: u8, material: String) -> Self {
        Self {
            slot_index,
            material,
            stack_count: 0,
            max_stack: 64,
        }
    }
    
    /// Check if slot has items
    pub fn has_items(&self) -> bool {
        self.stack_count > 0
    }
    
    /// Check if slot is empty
    pub fn is_empty(&self) -> bool {
        self.stack_count == 0
    }
    
    /// Add items to this slot (respects max stack)
    pub fn add_items(&mut self, count: u32) -> u32 {
        let space = self.max_stack.saturating_sub(self.stack_count);
        let actual_added = count.min(space);
        self.stack_count += actual_added;
        actual_added
    }
    
    /// Remove items from slot
    pub fn remove_items(&mut self, count: u32) -> u32 {
        let removed = count.min(self.stack_count);
        self.stack_count -= removed;
        removed
    }
}

/// Represents ender chests (obsidian storage)
#[derive(Debug, Clone)]
pub struct EndechestStorage {
    pub endchest_count: u32,
    pub obsidian_per_chest: u32,
    pub total_obsidian: u32,
}

impl EndechestStorage {
    pub fn new(endchest_count: u32, obsidian_per_chest: u32) -> Self {
        Self {
            endchest_count,
            obsidian_per_chest,
            total_obsidian: endchest_count * obsidian_per_chest,
        }
    }
    
    /// Calculate how many ender chests needed for X obsidian
    pub fn chests_needed_for(obsidian_count: u32, per_chest: u32) -> u32 {
        (obsidian_count + per_chest - 1) / per_chest // Ceiling division
    }
    
    /// Extract obsidian from ender chests
    pub fn extract_obsidian(&mut self, count: u32) -> u32 {
        let extracted = count.min(self.total_obsidian);
        self.total_obsidian -= extracted;
        extracted
    }
    
    /// Replenish ender chests
    pub fn refill(&mut self, new_count: u32, per_chest: u32) {
        self.endchest_count = new_count;
        self.obsidian_per_chest = per_chest;
        self.total_obsidian = new_count * per_chest;
    }
}

/// Main inventory tracker
pub struct BotInventory {
    pub shulker_slots: Vec<ShulkerSlot>, // 0-8
    pub endchest_storage: EndechestStorage,
    pub tool_durability: HashMap<String, u32>, // Track netherite tool wear
}

impl BotInventory {
    pub fn new(obsidian_per_chest: u32) -> Self {
        let mut shulker_slots = vec![];
        for i in 0..9 {
            shulker_slots.push(ShulkerSlot::new(i, String::new()));
        }
        
        Self {
            shulker_slots,
            endchest_storage: EndechestStorage::new(0, obsidian_per_chest),
            tool_durability: HashMap::new(),
        }
    }
    
    /// Check if inventory has specific material and quantity
    pub fn has_material(&self, material: &str, min_count: u32) -> bool {
        let mut total = 0;
        
        // Check shulkers
        for slot in &self.shulker_slots {
            if slot.material == material {
                total += slot.stack_count;
            }
        }
        
        // Check ender chests (only for obsidian)
        if material == "obsidian" {
            total += self.endchest_storage.total_obsidian;
        }
        
        total >= min_count
    }
    
    /// Get total count of material across all storage
    pub fn count_material(&self, material: &str) -> u32 {
        let mut total = 0;
        
        for slot in &self.shulker_slots {
            if slot.material == material {
                total += slot.stack_count;
            }
        }
        
        if material == "obsidian" {
            total += self.endchest_storage.total_obsidian;
        }
        
        total
    }
    
    /// Check if inventory is low (needs refill)
    /// Returns true if 50% or more slots are empty
    pub fn is_low(&self) -> bool {
        let empty_slots = self.shulker_slots.iter().filter(|s| s.is_empty()).count();
        empty_slots as f32 / 9.0 >= 0.5
    }
    
    /// Get percentage of inventory fullness
    pub fn fullness_percentage(&self) -> u32 {
        let filled_slots = self.shulker_slots.iter().filter(|s| s.has_items()).count();
        ((filled_slots as f32 / 9.0) * 100.0) as u32
    }
    
    /// Add items to first available slot for material
    pub fn add_to_inventory(&mut self, material: String, count: u32) -> bool {
        // Find slot with this material or empty slot
        for slot in &mut self.shulker_slots {
            if slot.material.is_empty() {
                slot.material = material;
                slot.add_items(count);
                return true;
            } else if slot.material == material {
                slot.add_items(count);
                return true;
            }
        }
        
        false // Inventory full
    }
    
    /// Remove material from inventory
    pub fn remove_from_inventory(&mut self, material: &str, count: u32) -> u32 {
        let mut removed = 0;
        
        for slot in &mut self.shulker_slots {
            if slot.material == material && removed < count {
                let take = (count - removed).min(slot.stack_count);
                removed += slot.remove_items(take);
            }
        }
        
        // Special case: obsidian can also come from ender chests
        if material == "obsidian" && removed < count {
            let from_ec = self.endchest_storage.extract_obsidian(count - removed);
            removed += from_ec;
        }
        
        removed
    }
    
    /// Reset inventory (after death or special event)
    pub fn reset(&mut self) {
        for slot in &mut self.shulker_slots {
            slot.stack_count = 0;
            slot.material.clear();
        }
    }
    
    /// Get inventory status string
    pub fn status_string(&self) -> String {
        let mut contents = String::new();
        
        for slot in &self.shulker_slots {
            if slot.has_items() {
                contents.push_str(&format!(
                    "\n  [Slot {}] {}: {}/{}",
                    slot.slot_index, slot.material, slot.stack_count, slot.max_stack
                ));
            }
        }
        
        if self.endchest_storage.total_obsidian > 0 {
            contents.push_str(&format!(
                "\n  [Ender Chests] count: {}, obsidian: {}",
                self.endchest_storage.endchest_count, self.endchest_storage.total_obsidian
            ));
        }
        
        format!(
            "[INVENTORY] Fullness: {}%{}",
            self.fullness_percentage(),
            contents
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shulker_slot() {
        let mut slot = ShulkerSlot::new(0, "stone".to_string());
        assert!(slot.is_empty());
        
        slot.add_items(64);
        assert_eq!(slot.stack_count, 64);
        assert!(slot.has_items());
        
        let removed = slot.remove_items(32);
        assert_eq!(removed, 32);
        assert_eq!(slot.stack_count, 32);
    }
    
    #[test]
    fn test_inventory_material_tracking() {
        let mut inventory = BotInventory::new(8);
        
        assert!(inventory.add_to_inventory("stone".to_string(), 64));
        assert_eq!(inventory.count_material("stone"), 64);
        assert!(inventory.has_material("stone", 32));
        assert!(!inventory.has_material("stone", 100));
    }
    
    #[test]
    fn test_inventory_low() {
        let inventory = BotInventory::new(8);
        assert!(inventory.is_low()); // Empty = low
    }
}
