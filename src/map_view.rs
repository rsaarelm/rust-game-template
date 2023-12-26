use content::{Level, Zone};
use engine::prelude::*;
use glam::{ivec3, IVec3};
use navni::{prelude::*, X256Color as X};
use ui::{prelude::*, render_fog, DisplayTile, SectorView};
use util::{v2, PolyLineIter};

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

    let view = SectorView::new(win.dim(), camera);

    // Snap camera to view center, which might have been adjusted by
    // SectorView.
    let camera = view.center(win.dim());

    if camera != game().camera {
        // Camera has moved, return this as the action unless we end up with a
        // better one
        ret = Some(RepositionCamera(camera));
    }

    let sector_area = {
        let bounds = Level::sector_from(&camera).wide();

        Rect::new(
            view.project(bounds.min()),
            view.project(bounds.max()) - ivec2(1, 0),
        )
    };

    for (p, loc) in view.iter(win.dim()) {
        DisplayTile::new(game(), loc).render(win, p);

        if let Some(e) = loc.snap_above_floor(&game().r).item_at(game()) {
            let cell = CharCell::c(e.icon(r));
            win.put(p, cell);
        }

        if let Some(e) = loc.snap_above_floor(&game().r).mob_at(game()) {
            let mut cell = CharCell::c(e.icon(r));
            if e.is_player_aligned(game()) {
                if game().r.player() == Some(e) {
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
            win.put(p, cell);
        }
    }

    // Ground animations are hidden under fog of war.
    game().draw_ground_anims(&win, view);

    for (p, loc) in view.iter(win.dim()) {
        render_fog(game(), win, p, loc);
    }

    // Cover up area outside the sector if viewport is big enough to show it.
    for p in win.area() {
        if !sector_area.contains(p) {
            win.put(p, CharCell::c('█').col(X::BROWN));
        }
    }

    // Sky animations are shown above fog of war.
    game().draw_sky_anims(&win, view);

    // Project locations of planned path and use polyline to fill in the gaps
    // to make a continuous line.
    for p in PolyLineIter::new(
        game().planned_path.posns().iter().map(|&a| view.project(a)),
    ) {
        if let Some(c) = win.get_mut(p) {
            c.invert();
        }
    }

    let mut mouse = navni::mouse_state();
    // Adjust coordinates from screen to window.
    mouse -= win.origin();

    if sector_area.contains(mouse) && game().current_active().is_some() {
        match mouse {
            MouseState::Hover(p) => {
                // This is a low-priority event, so don't overwrite an
                // existing ret.
                if ret.is_none() {
                    ret = Some(HoverOver(view.unproject_1(p)));
                }
            }

            MouseState::Drag(p, q, MouseButton::Left) if win.contains(q) => {
                // Marquee drag in progress, draw box.
                if p != q && sector_area.contains(p) && sector_area.contains(q)
                {
                    for c in Rect::from_points_inclusive([p, q])
                        .into_iter()
                        .filter_map(|p| win.get_mut(p))
                    {
                        c.invert();
                    }

                    // Planned path and marquee select are mutually exclusive
                    // visualizations.
                    game().planned_path.clear();
                }
            }

            MouseState::Release(p, q, MouseButton::Left) => {
                // Was this a local click or the end result of a drag?
                if p == q {
                    // Left click.
                    let loc = view.unproject_1(p);

                    let origin = game()
                        .current_active()
                        .and_then(|p| p.loc(game()))
                        .unwrap_or(loc);

                    match loc.mob_at(r) {
                        Some(npc) if npc.is_player_aligned(r) => {
                            // Select NPC or player.
                            ret = Some(SelectActive(vec![npc]));
                        }
                        Some(_enemy) if game().player_is_selected() => {
                            // Player group gets a move command that gets
                            // transformed into autofight when near enough.
                            ret = Some(Order(Goal::GoTo {
                                origin,
                                destination: Cube::unit(loc),
                                is_attack_move: false,
                            }));
                        }
                        Some(enemy) => {
                            // NPCs get a direct kill task instead.
                            ret = Some(Order(Goal::Attack(enemy)));
                        }
                        None => {
                            // Move to location.
                            ret = Some(Order(Goal::GoTo {
                                origin,
                                destination: Cube::unit(loc),
                                is_attack_move: false,
                            }));
                        }
                    }
                } else {
                    // A drag ended. Collect covered friendly units into
                    // selection.
                    if sector_area.contains(p) && sector_area.contains(q) {
                        ret = Some(SelectActive(
                            view.view_rect_locations(p, q)
                                .into_iter()
                                .filter_map(|loc| loc.mob_at(game()))
                                .filter(|e| e.is_player_aligned(game()))
                                .collect(),
                        ));
                    }
                }
            }

            MouseState::Release(p, q, MouseButton::Right) => {
                if p == q {
                    // Right click.
                    let loc = view.unproject_1(p);

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
    } else {
        game().planned_path.clear();
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
