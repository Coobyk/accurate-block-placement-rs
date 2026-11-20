// The rebindable toggle key (registered by `Hooks::register`, unbound by
// default as upstream). Repeats are left to the game.
fn abp_on_key(game: &mut Game<'_>, key: &str, action: mod_api::KeyAction) {
    if action == mod_api::KeyAction::Press {
        crate::game::abp_native::abp_key(game, key, true);
    }
}
