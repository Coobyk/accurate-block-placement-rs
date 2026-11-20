// Once a client tick, before the window's input runs: the oldest queued fresh
// target becomes the tick's held use (via `forced_hit`), and a target this
// hold already placed keeps the four-tick repeat closed.
fn abp_on_tick(game: &mut Game<'_>) {
    crate::game::abp_native::abp_tick(game);
}
