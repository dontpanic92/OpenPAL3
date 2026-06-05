use shared::GameType;

use crate::application::{boot_for, run_app};

pub fn run_openswd5() {
    // Preserve the legacy quirk: read the SWD5 asset path but
    // dispatch as SWDHC. `boot_for(SWDHC)` resolves via
    // `YaobowConfig::asset_path_for(SWDHC)` first, then falls back
    // to the hardcoded SWDHC dev path. Historically `run_openswd5`
    // read the SWD5 config key directly; if that distinction
    // matters for your install, set the SWDHC key instead.
    run_app(boot_for(GameType::SWDHC));
}
