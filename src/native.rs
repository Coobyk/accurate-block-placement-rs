//! Accurate Block Placement's adapter to the game (Horizon f00125f, game
//! 26.3). Injected as `crate::game::abp_native`.
//!
//! The mod in one sentence: while the use button is held, every *fresh* block
//! the crosshair lands on is used immediately, instead of waiting out
//! vanilla's four-tick `use_delay` repeat. Staring at a block already placed
//! on places nothing more (the delay is held shut), a single click stays a
//! single placement, and stepping a full block away makes targets fresh again.
//!
//! How: a per-frame pass queues fresh `Game::pick` hits. Once a tick, before
//! the window's input runs, the oldest queued hit becomes the tick's use —
//! `Game::forced_hit` aims `pick()` at it and `use_delay` opens — so the
//! game's own held-use gate does the placing: the same block-use packet and
//! prediction a press would send, on the local world and on servers alike.
//! When nothing is queued, a fresh block under the crosshair opens the gate
//! and is marked placed; a seen one holds it shut.

use super::hit::Hit;
use super::{Game, Screen};
use horizon_client::items::ItemStack;
use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};

/// The mod's id: its settings folder and the prefix of its key mappings.
const ID: &str = "accurate_block_placement";
/// The registered toggle key's name (`key.accurate_block_placement.toggle`).
const TOGGLE_KEY: &str = "toggle";
/// Queued targets one hold may keep. A target past the cap is not lost: it is
/// simply not marked seen, so it still opens the gate when hovered again.
const QUEUE_CAP: usize = 16;
/// The value `use_delay` is pinned to on a target this hold already placed.
/// The tick's decrement brings it to 1, under the gate's `== 0` test.
const SUPPRESS_DELAY: u32 = 2;

/// The hold's tracking. Cleared when the use button lets go, when the player
/// steps a block away, or when the mod stops applying.
#[derive(Default)]
struct State {
    /// Fresh hits the frame pass recorded, oldest first; one is placed a tick.
    queue: VecDeque<Hit>,
    /// The `(pos, face)` pairs this hold placed or queued.
    seen: HashSet<([i32; 3], [i32; 3])>,
    /// The block the player stood in when the hold began.
    anchor: Option<[i32; 3]>,
    /// `Game::forced_hit` this tick is ours; `saved` is what it was.
    forced: bool,
    saved: Option<Hit>,
    /// The use button was held on the last frame sample.
    held: bool,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
/// Never held while the game runs.
fn state<T>(run: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| run(&mut s.borrow_mut()))
}

/// A registered setting read fresh: the page writes them directly, so nothing
/// is cached here and a change applies at once.
fn flag(game: &Game<'_>, key: &str, default: bool) -> bool {
    game.setting(ID, key).map_or(default, |v| v == "true")
}

/// The configuration in effect. Broken values fall back to the defaults.
struct Config {
    enabled: bool,
    on_servers: bool,
    buckets: bool,
    armor_stands: bool,
    item_frames: bool,
    spawn_eggs: bool,
    tools: bool,
}
fn config(game: &Game<'_>) -> Config {
    Config {
        enabled: flag(game, "enabled", true),
        on_servers: flag(game, "allowed_on_servers", true),
        buckets: flag(game, "buckets", true),
        armor_stands: flag(game, "armor_stands", true),
        item_frames: flag(game, "item_frames", true),
        spawn_eggs: flag(game, "spawn_eggs", true),
        tools: flag(game, "tools", true),
    }
}

/// Whether the held-use pass runs this moment: the mod on, in play, use held,
/// no item in use (eating, drawing a bow), and an applicable stack in a hand.
fn active(game: &Game<'_>, cfg: &Config) -> bool {
    if !cfg.enabled
        || !game.right
        || !game.playing()
        || game.dialog_open()
        || game.sleeping.is_some()
        || game.player.is_none()
        || game.using.is_some()
    {
        return false;
    }
    if game.remote.is_some() && !cfg.on_servers {
        return false;
    }
    applies(game, cfg)
}

/// A held stack the mod may place: block items under the main toggle, the
/// other placeables under theirs (the Java mod's config, where an item is a
/// `BlockItem`, a bucket, an armor stand, an item frame, a spawn egg, or a
/// `block_transformer` tool — an axe, hoe or shovel).
fn applies(game: &Game<'_>, cfg: &Config) -> bool {
    let main = game.inventory.get(game.selected).and_then(|s| s.as_ref());
    let off = game.offhand.as_ref();
    main.into_iter()
        .chain(off)
        .any(|stack| item_applies(game, cfg, stack))
}

fn item_applies(game: &Game<'_>, cfg: &Config, stack: &ItemStack) -> bool {
    let Some(info) = game.items().get(&stack.item) else {
        return false;
    };
    if info.block.is_some() {
        // Block items answer to the main toggle (`enabled` is already on by
        // the time this runs, but the item's own rule stays explicit).
        return cfg.enabled;
    }
    if info.block_transformer.is_some() {
        return cfg.tools;
    }
    let name = &*info.id;
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    if short.ends_with("_spawn_egg") {
        cfg.spawn_eggs
    } else if short == "armor_stand" {
        cfg.armor_stands
    } else if short == "item_frame" || short == "glow_item_frame" {
        cfg.item_frames
    } else if short.ends_with("_bucket") && short != "milk_bucket" {
        // Buckets that place: water, lava, powder snow and the mob buckets.
        // The milk bucket only drinks.
        cfg.buckets
    } else {
        false
    }
}

/// The crosshair aims at an entity the use belongs to (breeding, trading,
/// mounting): leave the game to it, queue intact for when the aim moves on.
fn entity_targeted(game: &Game<'_>) -> bool {
    if game.remote.is_some() {
        game.remote_use_targets_entity()
    } else {
        game.entity_targeted()
    }
}

/// Scaffolding is the one block vanilla keeps placing while it is stared at
/// (`ScaffoldingBlockItem` re-picks through it); leave it to the game's
/// repeat rather than the mod's once-per-target rule.
fn reactive(game: &Game<'_>, hit: &Hit) -> bool {
    game.block_name_plain((hit.pos[0], hit.pos[1], hit.pos[2])).as_deref()
        == Some("minecraft:scaffolding")
}

/// The block the player stands in; stepping one block from the anchor makes
/// every target fresh again (the Forge original's backward-step rule).
fn player_block(game: &Game<'_>) -> Option<[i32; 3]> {
    let p = game.player.as_ref()?;
    Some([
        p.position.x.floor() as i32,
        p.position.y.floor() as i32,
        p.position.z.floor() as i32,
    ])
}

/// The key the player bound the toggle to (Options > Controls), `key` events
/// being named physical keys (`key.keyboard.g`).
fn toggle_key(game: &Game<'_>) -> String {
    game.mod_key(ID, TOGGLE_KEY)
        .map(|(key, _)| key)
        .unwrap_or_else(|| ::accurate_block_placement::UNBOUND.to_owned())
}

/// Once a frame (the mixin on `Game::native_render_frame_pre`). While use is
/// held, a fresh block under the crosshair joins the placement queue; the
/// rising edge instead marks the block the press itself is already on, so the
/// ordinary click it makes counts as that target's placement. Queueing does
/// not check `entity_targeted`: `abp_tick` early-outs on an entity aim
/// without popping, so queued hits simply wait for the aim to move on.
pub(in crate::game) fn abp_frame(game: &mut Game<'_>) {
    let cfg = config(game);
    state(|s| {
        if !active(game, &cfg) {
            s.held = false;
            s.seen.clear();
            s.queue.clear();
            s.anchor = None;
            return;
        }
        if !s.held {
            s.held = true;
            s.anchor = player_block(game);
            if let Some(hit) = game.pick() {
                s.seen.insert((hit.pos, hit.face));
            }
            return;
        }
        if let Some(pos) = player_block(game) {
            if s.anchor.is_some_and(|anchor| anchor != pos) {
                s.seen.clear();
                s.queue.clear();
                s.anchor = Some(pos);
            }
        }
        let Some(hit) = game.pick() else { return };
        if reactive(game, &hit) {
            return;
        }
        let key = (hit.pos, hit.face);
        if !s.seen.contains(&key) && s.queue.len() < QUEUE_CAP {
            s.seen.insert(key);
            s.queue.push_back(hit);
        }
    });
}

/// Once a client tick, before the window's input runs (the mixin on
/// `Game::native_client_tick_pre`). The oldest queued fresh target becomes
/// the tick's use; with an empty queue, an unseen block under the crosshair
/// opens the gate and a seen one keeps it shut.
pub(in crate::game) fn abp_tick(game: &mut Game<'_>) {
    let cfg = config(game);
    state(|s| {
        // A `forced_hit` older than a tick is a leftover (`tick_post` missed):
        // aiming clicks at a stale block is worse than losing one placement.
        if s.forced {
            game.forced_hit = s.saved;
            s.saved = None;
            s.forced = false;
        }
        if !active(game, &cfg) || entity_targeted(game) {
            return;
        }
        if let Some(hit) = s.queue.pop_front() {
            if reactive(game, &hit) {
                return;
            }
            s.saved = game.forced_hit;
            game.forced_hit = Some(hit);
            s.forced = true;
            game.use_delay = 0;
            return;
        }
        let Some(hit) = game.pick() else { return };
        if reactive(game, &hit) {
            return;
        }
        if s.seen.insert((hit.pos, hit.face)) {
            game.use_delay = 0;
        } else {
            game.use_delay = game.use_delay.max(SUPPRESS_DELAY);
        }
    });
}

/// After the client tick (the mixin on `Game::native_client_tick_post`): the
/// hit the tick's use was aimed at is put back for whoever set it before us
/// (a hosted world aims a guest's hits the same way).
pub(in crate::game) fn abp_post(game: &mut Game<'_>) {
    state(|s| {
        if s.forced {
            game.forced_hit = s.saved;
            s.saved = None;
            s.forced = false;
        }
    });
}

/// The registered toggle key (the mixin on `Game::native_input_key`): it flips
/// the persisted `enabled` setting the page shows, with a line in chat.
pub(in crate::game) fn abp_key(game: &mut Game<'_>, key: &str) {
    if game.screen != Screen::Playing || key != toggle_key(game) {
        return;
    }
    let on = !flag(game, "enabled", true);
    game.set_setting(ID, "enabled", if on { "true" } else { "false" });
    let message = horizon_client::text::Text::tr(
        if on {
            "accurate_block_placement.toggle.on"
        } else {
            "accurate_block_placement.toggle.off"
        },
        &[],
    );
    game.chat_line(message);
}
