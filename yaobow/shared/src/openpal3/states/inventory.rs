use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: u32,
    pub count: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryState {
    stacks: Vec<ItemStack>,
}

impl InventoryState {
    pub fn stacks(&self) -> &[ItemStack] {
        &self.stacks
    }

    pub fn contains(&self, id: u32) -> bool {
        self.stacks.iter().any(|stack| stack.id == id)
    }

    pub fn quantity(&self, id: u32) -> i32 {
        self.stacks
            .iter()
            .find(|stack| stack.id == id)
            .map_or(0, |stack| stack.count)
    }

    pub(crate) fn add_stack(&mut self, id: u32, count: i32) {
        if let Some(stack) = self.stacks.iter_mut().find(|stack| stack.id == id) {
            // Native 0x40cf36 caps only existing stacks, after signed wrapping addition.
            stack.count = stack.count.wrapping_add(count).min(99);
        } else {
            self.stacks.push(ItemStack { id, count });
        }
    }

    pub(crate) fn remove_one(&mut self, id: u32) {
        if let Some(index) = self.stacks.iter().position(|stack| stack.id == id) {
            self.stacks[index].count = self.stacks[index].count.wrapping_sub(1);
            if self.stacks[index].count <= 0 {
                self.stacks.remove(index);
            }
        }
    }
}

pub(super) fn linked_items(id: u32) -> &'static [u32] {
    match id {
        6119 => &[8146],
        6120 => &[8147],
        6122 => &[8149],
        6123 => &[8150, 6427],
        6124 => &[8151],
        6125 => &[8152],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_insertion_merge_and_removal_preserve_native_order_and_limits() {
        let mut bag = InventoryState::default();
        bag.add_stack(1, 200);
        bag.add_stack(2, 2);
        assert_eq!(bag.quantity(1), 200);
        bag.add_stack(1, 1);
        assert_eq!(bag.quantity(1), 99);
        bag.remove_one(2);
        assert_eq!(bag.quantity(2), 1);
        bag.remove_one(2);
        assert!(!bag.contains(2));
        bag.add_stack(2, 1);
        assert_eq!(
            bag.stacks(),
            &[
                ItemStack { id: 1, count: 99 },
                ItemStack { id: 2, count: 1 }
            ]
        );
        bag.add_stack(1, i32::MAX);
        assert_eq!(bag.quantity(1), i32::MIN + 98);
        bag.remove_one(1);
        assert!(!bag.contains(1));
    }

    #[test]
    fn presence_checks_stack_identity_even_for_zero_and_negative_counts() {
        let mut bag = InventoryState::default();
        bag.add_stack(1, 0);
        bag.add_stack(2, -2);
        assert!(bag.contains(1));
        assert!(bag.contains(2));
        bag.add_stack(2, 1);
        assert_eq!(bag.quantity(2), -1);
        bag.remove_one(1);
        assert!(!bag.contains(1));
        bag.remove_one(999);
        assert_eq!(bag.stacks().len(), 1);
    }
}
