// Once a rendered frame: a fresh block under a held-use crosshair joins the
// placement queue a tick places from. Per-frame sampling is what keeps a fast
// sweep from skipping blocks the tick's own pick no longer sees.
fn abp_on_frame(game: &mut Game<'_>) {
    crate::game::abp_native::abp_frame(game);
}
