use opengb::directors::{GlobalState, SceProcHooks};
use std::collections::HashMap;

pub struct SceRestHooks {
    g_story_updates: HashMap<String, HashMap<u32, HashMap<i32, i32>>>,
}

impl SceRestHooks {
    pub const GSTORY_ID: i16 = -32768;
    pub fn new() -> SceRestHooks {
        let g_story_updates = make_map(&[(
            "Q01".to_string(),
            make_map(&[(1415, make_map(&[(11101, 11200), (120301, 120302)]))]),
        )]);

        SceRestHooks { g_story_updates }
    }
}

impl SceProcHooks for SceRestHooks {
    fn proc_begin(&self, _sce_name: &str, _proc_id: u32, _global_state: &mut GlobalState) {}

    fn proc_end(&self, sce_name: &str, proc_id: u32, global_state: &mut GlobalState) {
        if let Some(procs) = self.g_story_updates.get(&sce_name.to_uppercase()) {
            if let Some(updates) = procs.get(&proc_id) {
                let g_story = global_state
                    .persistent_state()
                    .get_global(SceRestHooks::GSTORY_ID)
                    .unwrap_or(-1);

                if let Some(new_value) = updates.get(&g_story) {
                    global_state
                        .persistent_state_mut()
                        .set_global(SceRestHooks::GSTORY_ID, *new_value);
                }
            }
        }
    }
}

fn make_map<TKey: Clone + Eq + std::hash::Hash, TValue: Clone>(
    slice: &[(TKey, TValue)],
) -> HashMap<TKey, TValue> {
    slice.iter().cloned().collect()
}
