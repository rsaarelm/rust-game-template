use engine::prelude::*;
use glam::{ivec3, IVec3};
use navni::{prelude::*, X256Color as X};
use ui::prelude::*;
use util::v2;

#[derive(Clone, Debug)]
pub enum MapAction {
    /// Give the active character a direct command.
    DirectCommand(Action),

    /// Give all selected characters an indirect order.
    Order(Goal),

    /// Move the camera position.
    ///
    /// Can be the result of a scroll action or result of snapping to window
    /// bounds.
    RepositionCamera(Location),

    /// Mouse hovers over ground, plot a path or show information about a mob.
    HoverOver(Location),

    /// Select any number of entities or clear selection with an empty list.
    SelectActive(Vec<Entity>),

    BecomePlayer(Entity),
    NextEntity,
}

use MapAction::*;

// NB. Even though map_view accesses the game singleton (because I'm too lazy
// to set it up as a widget that would take references to all the subsystems
// it needs to run), it must NOT effect any mutable change on its own. All
// changes are communicated via the renturn value.

pub fn view_map(win: &Window) -> Option<MapAction> {
    const SCROLL_STEP: i32 = 4;

    // The output event.
    let mut ret = None;

    let r = &game().r;

    let mut camera = game().camera;

    // Get scroll input.
    let mut scroll = match input_press() {
        Some(InputAction::ScrollNorth) => ivec3(0, -1, 0),
        Some(InputAction::ScrollEast) => ivec3(1, 0, 0),
        Some(InputAction::ScrollSouth) => ivec3(0, 1, 0),
        Some(InputAction::ScrollWest) => ivec3(-1, 0, 0),
        _ => Default::default(),
    };
    if scroll == IVec3::ZERO {
        scroll = v2(navni::mouse_state().scroll_delta()).extend(0);
    }
    camera += scroll * SCROLL_STEP;

    let wide_sector_bounds = wide_unfolded_sector_bounds(camera);
    let offset =
        scroll_offset(&win.area(), camera.unfold_wide(), &wide_sector_bounds);

    // Snap camera to view center.
    let camera =
        Location::fold_wide_sides(offset + v2(win.area().dim()) / ivec2(2, 2))
            .0;

    // Calculate offset again with new camera to cover for off-by-one problems
    // from the half-cells.
    let offset =
        scroll_offset(&win.area(), camera.unfold_wide(), &wide_sector_bounds);

    // Camera has moved, return this as the action unless we end up with a
    // better one
    if camera != game().camera {
        ret = Some(RepositionCamera(camera));
    }

    // Solid background for off-sector extra space.
    win.fill(CharCell::c('█').col(X::BROWN));
    // Constrain sub-window to current sector only.
    let sector_win = win.sub(wide_sector_bounds - offset);
    // Adjust offset for sub-window position.
    let offset = v2(wide_sector_bounds.min()).max(offset);

    // Draw main map contents, animations and fog of war.
    draw_map(r, &sector_win, offset);
    game().draw_ground_anims(&sector_win, offset);
    draw_fog(r, &sector_win, offset);
    game().draw_sky_anims(&sector_win, offset);

    // Highlight planned path.
    for &p in game().planned_path.posns() {
        if let Some(c) = sector_win.get_mut(p - offset) {
            c.invert();
        }
    }

    // Coordinate space helpers.
    let screen_to_wide_pos = |screen_pos: [i32; 2]| {
        v2(screen_pos) - v2(sector_win.bounds().min()) + offset
    };

    let screen_to_loc_pos = |screen_pos: [i32; 2]| {
        // Get wide location pos corresponding to screen space pos.
        let wide_pos = screen_to_wide_pos(screen_pos);
        // Snap to cell.
        ivec2(wide_pos.x.div_euclid(2), wide_pos.y)
    };

    // Get a click target, preferring cells with mobs in them.
    let click_target = |r: &Runtime, wide_pos: IVec2| -> Location {
        let (a, b) = Location::fold_wide_sides(wide_pos);
        // Prefer left cell unless right has a mob and left doesn't.
        if b.mob_at(r).is_some() && a.mob_at(r).is_none() {
            b
        } else {
            a
        }
    };

    let mouse = navni::mouse_state();
    if win.contains(mouse) && game().current_active().is_some() {
        match mouse {
            MouseState::Hover(p) => {
                // This is a low-priority event, so don't overwrite an
                // existing ret.
                if ret.is_none() {
                    ret = Some(HoverOver(Location::smart_fold_wide(
                        screen_to_wide_pos(p),
                        r,
                    )));
                }
            }

            MouseState::Drag(p, q, MouseButton::Left) if win.contains(q) => {
                let (a, b) = (screen_to_loc_pos(q), screen_to_loc_pos(p));

                // Marquee drag in progress, draw box.
                if wide_sector_bounds.contains(a)
                    && wide_sector_bounds.contains(b)
                {
                    for c in Rect::from_points([p, q])
                        .into_iter()
                        .map(|sp| v2(sp) - v2(sector_win.bounds().min()))
                        .filter_map(|p| sector_win.get_mut(p))
                    {
                        c.invert();
                    }
                }
            }

            MouseState::Release(p, q, MouseButton::Left) => {
                // Was this a local click or the end result of a drag?
                let (a, b) = (screen_to_wide_pos(q), screen_to_wide_pos(p));
                if a == b {
                    // Left click.
                    let loc = click_target(r, a);

                    match loc.mob_at(r) {
                        Some(npc) if npc.is_player_aligned(r) => {
                            // Select NPC or player.
                            ret = Some(SelectActive(vec![npc]));
                        }
                        Some(_enemy) if game().player_is_selected() => {
                            // Player group gets a move command that gets
                            // transformed into autofight when near enough.
                            ret = Some(Order(Goal::GoTo(loc)));
                        }
                        Some(enemy) => {
                            // NPCs get a direct kill task instead.
                            ret = Some(Order(Goal::Attack(enemy)));
                        }
                        None => {
                            // Move to location.
                            ret = Some(Order(Goal::GoTo(loc)));
                        }
                    }
                } else {
                    // A drag ended. Collect covered friendly units into
                    // selection.
                    if wide_sector_bounds.contains(a)
                        && wide_sector_bounds.contains(b)
                    {
                        ret = Some(SelectActive(
                            Rect::from_points([a, b])
                                .into_iter()
                                .filter_map(Location::fold_wide)
                                .filter_map(|loc| loc.mob_at(game()))
                                .filter(|e| e.is_player_aligned(game()))
                                .collect(),
                        ));
                    }
                }
            }

            MouseState::Release(p, q, MouseButton::Right) => {
                let (a, b) = (screen_to_wide_pos(q), screen_to_wide_pos(p));
                if a == b {
                    // Right click.
                    let loc = click_target(r, a);

                    match loc.mob_at(r) {
                        Some(npc) if npc.is_player_aligned(r) => {
                            // On a NPC, make that NPC become player.
                            ret = Some(BecomePlayer(npc));
                        }
                        Some(enemy) => {
                            // On an enemy, attack the enemy.
                            ret = Some(Order(Goal::Attack(enemy)));
                        }
                        None => {
                            // TODO: Shoot in direction
                        }
                    }
                }
            }

            _ => {}
        }
    }

    // Capture direct commands.
    if let Some(a) = input_press() {
        if let Ok(act) = Action::try_from(a) {
            ret = Some(DirectCommand(act));
        } else if a == InputAction::Cycle {
            ret = Some(NextEntity);
        } else if a == InputAction::BecomePlayer {
            if let Some(p) = game().current_active() {
                ret = Some(BecomePlayer(p));
            }
        }
    }

    ret
}

fn draw_map(r: &Runtime, win: &Window, offset: IVec2) {
    for draw_pos in win.area().into_iter().map(v2) {
        let p = draw_pos + offset;

        win.put(draw_pos, ui::flat_terrain_cell(r, p));

        if let Some(loc) = Location::fold_wide(p) {
            if let Some(e) = loc.mob_at(r) {
                let mut cell = CharCell::c(e.icon(r));
                if e.is_player_aligned(r) {
                    if r.player() == Some(e) {
                        cell.set_c('@');
                    } else if !e.can_be_commanded(r) {
                        // Friendly mob out of moves.
                        cell = cell.col(X::GRAY);
                    } else if e.goal(r) != Goal::FollowPlayer {
                        // Frindly mob out on a mission.
                        cell = cell.col(X::GREEN);
                    } else if e.acts_before_next_player_frame(r) {
                        // Friendly mob ready for next command
                        cell = cell.col(X::AQUA);
                    } else {
                        // Friendly mob still building up it's actions.
                        cell = cell.col(X::TEAL);
                    }

                    if game().selected().any(|a| a == e) {
                        cell = cell.inv();
                    }
                }
                win.put(draw_pos, cell);
            } else if let Some(e) = loc.item_at(r) {
                win.put(draw_pos, CharCell::c(e.icon(r)));
            }
        }
    }
}

fn draw_fog(r: &Runtime, win: &Window, offset: IVec2) {
    for draw_pos in win.area().into_iter().map(v2) {
        if r.wide_pos_is_shrouded(draw_pos + offset) {
            win.put(draw_pos, CharCell::c('░').col(X::BROWN));
        }
    }
}

/// Rectangle containing cells of location's sector plus one-cell rim of
/// adjacent sectors projected into wide unfolded space.
fn wide_unfolded_sector_bounds(loc: Location) -> Rect {
    // Get sector area with the rim to adjacent sectors.
    let bounds = loc.expanded_sector_bounds();

    // Convert to wide space.
    let p1 = IVec2::from(bounds.min()) * ivec2(2, 1);
    let mut p2 = IVec2::from(bounds.max()) * ivec2(2, 1);

    // Trim out the part that would be in-between cells for cells that don't
    // belong in the original set.
    p2.x = 0.max(p2.x - 1);

    Rect::new(p1, p2)
}

/// Compute an offset to add to canvas rectangle points to show map rectangle
/// points.
///
/// Offsetting will try to ensure maximum amount of map is shown on canvas. If
/// the map center is near map rectangle's edge, map rectangle will be offset
/// so it's edge will snap the inside of the canvas rectangle. If the map
/// rectangle is smaller than the canvas rectangle along either dimension, it
/// can't fill the canvas rectangle and will be centered on the canvas
/// rectangle instead along that dimension.
fn scroll_offset(
    canvas_rect: &Rect,
    view_pos: IVec2,
    map_rect: &Rect,
) -> IVec2 {
    // Starting point, snap to the center of the canvas.
    let mut offset = view_pos - IVec2::from(canvas_rect.center());

    let offset_rect = *map_rect - offset;

    // Check each axis
    for d in 0..2 {
        if offset_rect.dim()[d] < canvas_rect.dim()[d] {
            // Canvas is big enough (along this axis) to fit the whole arena.
            // Just center the arena rect then.
            offset[d] = map_rect.min()[d] - canvas_rect.min()[d]
                + (map_rect.dim()[d] - canvas_rect.dim()[d]) / 2;
        } else if offset_rect.min()[d] > canvas_rect.min()[d] {
            // Snap inside inner edge of the canvas_rect.
            offset[d] += offset_rect.min()[d] - canvas_rect.min()[d];
        } else if offset_rect.max()[d] < canvas_rect.max()[d] {
            // Snap inside outer edge of the canvas_rect.
            offset[d] -= canvas_rect.max()[d] - offset_rect.max()[d];
        }
    }

    offset
}
