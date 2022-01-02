use std::collections::{BTreeMap, BTreeSet};

use geom::Distance;
use map_model::{Block, Perimeter, RoadID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::ltn::partition::NeighborhoodID;
use crate::ltn::Neighborhood;

const SELECTED: Color = Color::CYAN;

pub struct SelectBoundary {
    panel: Panel,
    // These are always single, unmerged blocks. Thus, these blocks never change -- only their
    // color and assignment to a neighborhood.
    blocks: BTreeMap<BlockID, Block>,
    world: World<BlockID>,
    selected: BTreeSet<BlockID>,
    draw_outline: ToggleZoomed,
    block_to_neighborhood: BTreeMap<BlockID, NeighborhoodID>,
    frontier: BTreeSet<BlockID>,
    current_neighborhood: NeighborhoodID,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BlockID(usize);
impl ObjectID for BlockID {}

impl SelectBoundary {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        // TODO Take NeighborhoodID?
        initial_boundary: Perimeter,
    ) -> Box<dyn State<App>> {
        let mut state = SelectBoundary {
            panel: make_panel(ctx, app),
            blocks: BTreeMap::new(),
            world: World::bounded(app.primary.map.get_bounds()),
            selected: BTreeSet::new(),
            draw_outline: ToggleZoomed::empty(ctx),
            block_to_neighborhood: BTreeMap::new(),
            frontier: BTreeSet::new(),
            // Temporary, will assign below
            current_neighborhood: NeighborhoodID(usize::MAX),
        };

        ctx.loading_screen("calculate all blocks", |ctx, timer| {
            timer.start("find single blocks");
            let perimeters = Perimeter::find_all_single_blocks(&app.primary.map);
            timer.stop("find single blocks");

            let mut blocks = Vec::new();
            timer.start_iter("blockify", perimeters.len());
            for perimeter in perimeters {
                timer.next();
                match perimeter.to_block(&app.primary.map) {
                    Ok(block) => {
                        blocks.push(block);
                    }
                    Err(err) => {
                        warn!("Failed to make a block from a perimeter: {}", err);
                    }
                }
            }

            for (idx, block) in blocks.into_iter().enumerate() {
                let id = BlockID(idx);
                let neighborhood = app.session.partitioning.neighborhood_containing(&block);
                state.block_to_neighborhood.insert(id, neighborhood);
                if initial_boundary.contains(&block.perimeter) {
                    state.selected.insert(id);
                    state.current_neighborhood = neighborhood;
                }
                state.blocks.insert(id, block);
            }
            state.frontier = calculate_frontier(&initial_boundary, &state.blocks);

            // Fill out the world initially
            for id in state.blocks.keys().cloned().collect::<Vec<_>>() {
                state.add_block(ctx, app, id);
            }
        });

        state.world.initialize_hover(ctx);
        Box::new(state)
    }

    fn add_block(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        let color = if self.selected.contains(&id) {
            SELECTED
        } else {
            // Use the original color. This assumes the partitioning has been updated, of
            // course
            let neighborhood = self.block_to_neighborhood[&id];
            app.session.partitioning.neighborhoods[&neighborhood].1
        };

        if self.frontier.contains(&id) {
            let mut obj = self
                .world
                .add(id)
                .hitbox(self.blocks[&id].polygon.clone())
                .draw_color(color.alpha(0.5))
                .hover_alpha(0.8)
                .clickable();
            if self.selected.contains(&id) {
                obj = obj
                    .hotkey(Key::Space, "remove")
                    .hotkey(Key::LeftShift, "remove")
            } else {
                obj = obj
                    .hotkey(Key::Space, "add")
                    .hotkey(Key::LeftControl, "add")
            }
            obj.build(ctx);
        } else {
            // If we can't immediately add/remove the block, fade it out and don't allow clicking
            // it
            self.world
                .add(id)
                .hitbox(self.blocks[&id].polygon.clone())
                .draw_color(color.alpha(0.3))
                .build(ctx);
        }
    }

    fn merge_selected(&self) -> Vec<Perimeter> {
        let mut perimeters = Vec::new();
        for id in &self.selected {
            perimeters.push(self.blocks[&id].perimeter.clone());
        }
        Perimeter::merge_all(perimeters, false)
    }

    // This block was in the previous frontier; its inclusion in self.selected has changed.
    fn block_changed(&mut self, ctx: &mut EventCtx, app: &App, id: BlockID) {
        let mut perimeters = self.merge_selected();
        let maybe_new_block = perimeters
            .pop()
            .and_then(|perim| perim.to_block(&app.primary.map));
        if !perimeters.is_empty() || maybe_new_block.is_none() {
            let error = if !perimeters.is_empty() {
                "Splitting this neighborhood in two is currently unsupported"
            } else {
                // Why couldn't we blockify?
                "This change broke something internal"
            };
            // Revert!
            if self.selected.contains(&id) {
                self.selected.remove(&id);
            } else {
                self.selected.insert(id);
            }
            let label = error.text_widget(ctx);
            self.panel.replace(ctx, "warning", label);
            return;
        }

        if self.selected.contains(&id) {
            // We just "stole" a block from an adjacent neighborhood
            let old_neighborhood = self.block_to_neighborhood[&id];
            assert_ne!(old_neighborhood, self.current_neighborhood);
            self.block_to_neighborhood
                .insert(id, self.current_neighborhood);
            app.session
                .partitioning
                .neighborhoods
                .get_mut(&self.current_neighborhood)
                .unwrap()
                .0 = maybe_new_block.unwrap();
            // TODO We may need to recalculate the coloring of all the neighborhoods!

            // Recalculate the old neighborhood now
            // TODO We have its Perimeter and want to steal one block from it. Or more easily... we
            // have block_to_neighborhood and all the individual blocks, so do merge_all for it.
            // // TODO If this fails, revert everything. Getting messy, can we return a Result and
            // make the caller handle reverting?
        } else {
        }

        let old_frontier = std::mem::take(&mut self.frontier);
        self.frontier = calculate_frontier(&new_perimeter, &self.blocks);

        // Redraw all of the blocks that changed
        let mut changed_blocks: Vec<BlockID> = old_frontier
            .symmetric_difference(&self.frontier)
            .cloned()
            .collect();
        // And always the current block
        changed_blocks.push(id);
        for changed in changed_blocks {
            self.world.delete_before_replacement(changed);
            self.add_block(ctx, app, changed);
        }

        // Draw the outline of the current blocks
        let mut batch = ToggleZoomed::builder();
        if let Ok(block) = new_perimeter.to_block(&app.primary.map) {
            if let Ok(outline) = block.polygon.to_outline(Distance::meters(10.0)) {
                batch.unzoomed.push(Color::RED, outline);
            }
            if let Ok(outline) = block.polygon.to_outline(Distance::meters(5.0)) {
                batch.zoomed.push(Color::RED.alpha(0.5), outline);
            }
        }
        // TODO If this fails, maybe also revert
        self.draw_outline = batch.build(ctx);
        self.panel = make_panel(ctx, app);
    }
}

impl State<App> for SelectBoundary {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Cancel" => {
                    return Transition::Pop;
                }
                "Confirm" => {
                    let mut perimeters = self.merge_selected();
                    assert_eq!(perimeters.len(), 1);
                    // TODO Persist the partitioning
                    return Transition::Replace(super::connectivity::Viewer::new_state(
                        ctx,
                        app,
                        Neighborhood::new(ctx, app, perimeters.pop().unwrap()),
                    ));
                }
                _ => unreachable!(),
            }
        }

        match self.world.event(ctx) {
            WorldOutcome::Keypress("add", id) => {
                self.selected.insert(id);
                self.block_changed(ctx, app, id)
            }
            WorldOutcome::Keypress("remove", id) => {
                self.selected.remove(&id);
                self.block_changed(ctx, app, id)
            }
            WorldOutcome::ClickedObject(id) => {
                if self.selected.contains(&id) {
                    self.selected.remove(&id);
                } else {
                    self.selected.insert(id);
                }
                self.block_changed(ctx, app, id)
            }
            _ => {}
        }
        // TODO Bypasses World...
        if ctx.redo_mouseover() {
            if let Some(id) = self.world.get_hovering() {
                if ctx.is_key_down(Key::LeftControl) {
                    if !self.selected.contains(&id) {
                        self.selected.insert(id);
                        self.block_changed(ctx, app, id);
                    }
                } else if ctx.is_key_down(Key::LeftShift) {
                    if self.selected.contains(&id) {
                        self.selected.remove(&id);
                        self.block_changed(ctx, app, id);
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.world.draw(g);
        self.draw_outline.draw(g);
        self.panel.draw(g);
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    Panel::new_builder(Widget::col(vec![
        map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
        "Draw a custom boundary for a neighborhood"
            .text_widget(ctx)
            .centered_vert(),
        Text::from_all(vec![
            Line("Click").fg(ctx.style().text_hotkey_color),
            Line(" to add/remove a block"),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Hold "),
            Line(Key::LeftControl.describe()).fg(ctx.style().text_hotkey_color),
            Line(" and paint over blocks to add"),
        ])
        .into_widget(ctx),
        Text::from_all(vec![
            Line("Hold "),
            Line(Key::LeftShift.describe()).fg(ctx.style().text_hotkey_color),
            Line(" and paint over blocks to remove"),
        ])
        .into_widget(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_solid_primary
                .text("Confirm")
                .hotkey(Key::Enter)
                .build_def(ctx),
            ctx.style()
                .btn_solid_destructive
                .text("Cancel")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]),
        Text::new().into_widget(ctx).named("warning"),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
    .build(ctx)
}

// Blocks on the "frontier" are adjacent to the perimeter, either just inside or outside.
fn calculate_frontier(perim: &Perimeter, blocks: &BTreeMap<BlockID, Block>) -> BTreeSet<BlockID> {
    let perim_roads: BTreeSet<RoadID> = perim.roads.iter().map(|id| id.road).collect();

    let mut frontier = BTreeSet::new();
    for (block_id, block) in blocks {
        for road_side_id in &block.perimeter.roads {
            // If the perimeter has this RoadSideID on the same side, we're just inside. If it has
            // the other side, just on the outside. Either way, on the frontier.
            if perim_roads.contains(&road_side_id.road) {
                frontier.insert(*block_id);
                break;
            }
        }
    }
    frontier
}
