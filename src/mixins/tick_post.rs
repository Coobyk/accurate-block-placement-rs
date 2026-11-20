// After the client tick: the hit the tick's use was aimed at is a one-tick
// loan; `forced_hit` goes back to what it was (nothing, or a host's guest aim).
fn abp_on_post(game: &mut Game<'_>) {
    crate::game::abp_native::abp_post(game);
}
