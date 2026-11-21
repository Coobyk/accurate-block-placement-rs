//! Accurate Block Placement's own crate. The client mixins under `src/mixins/`
//! are compiled into copies of the game functions they patch; the shared
//! module `src/native.rs` is injected by `[[native.module]]` into the game's
//! `crate::game` as `abp_native`.
//!
//! Upstream is the Fabric mod AccurateBlockPlacement Reborn: while the use key
//! is held, every fresh block the crosshair lands on is used at once instead of
//! waiting out vanilla's four-tick repeat. Horizon draws and persists the
//! settings and the (unbound-by-default) toggle key itself; the game side reads
//! the same values with `Game::setting` / `Game::mod_key`.

use mod_api::{Hooks, KeyRegistration, Registrar, RegistrationContext, SettingSpec};

/// `KeyRegistration`'s default for an unbound mapping (vanilla's name for
/// one); the game side reads the same value from `crate::game::abp_native`.
pub const UNBOUND: &str = "key.keyboard.unknown";

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        // Upstream leaves the toggle unbound.
        r.register_key_mapping(
            KeyRegistration::new("toggle", UNBOUND)
                .label("key.accurate_block_placement.toggle"),
        );

        for (key, label, default) in [
            ("enabled", "accurate_block_placement.setting.enabled", true),
            (
                "allowed_on_servers",
                "accurate_block_placement.setting.allowed_on_servers",
                true,
            ),
            ("buckets", "accurate_block_placement.setting.buckets", true),
            (
                "armor_stands",
                "accurate_block_placement.setting.armor_stands",
                true,
            ),
            (
                "item_frames",
                "accurate_block_placement.setting.item_frames",
                true,
            ),
            (
                "spawn_eggs",
                "accurate_block_placement.setting.spawn_eggs",
                true,
            ),
            ("tools", "accurate_block_placement.setting.tools", true),
        ] {
            let _ = r.register_setting(SettingSpec::toggle(key, label, default));
        }
    }
}
