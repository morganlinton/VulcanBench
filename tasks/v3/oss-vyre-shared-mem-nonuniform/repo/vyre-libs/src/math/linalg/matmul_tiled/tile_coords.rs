//! Output-tile coordinate binding shared by cooperative and tensor-core bodies.

use vyre::ir::{Expr, Node};

use super::shape::{MatrixShape, TileShape};

pub(crate) struct OutputTileCoordNames {
    pub(crate) lane_row: &'static str,
    pub(crate) lane_col: &'static str,
    pub(crate) row: &'static str,
    pub(crate) col: &'static str,
}

pub(crate) fn bind_output_tile_coordinates(
    shape: MatrixShape,
    tile: TileShape,
    names: OutputTileCoordNames,
) -> Vec<Node> {
    let local = Expr::var("local");
    vec![
        Node::let_bind(
            "local",
            Expr::add(
                Expr::add(
                    Expr::LocalId { axis: 0 },
                    Expr::mul(Expr::LocalId { axis: 1 }, Expr::u32(tile.x_lanes)),
                ),
                Expr::mul(
                    Expr::LocalId { axis: 2 },
                    Expr::u32(tile.x_lanes.saturating_mul(tile.y_lanes)),
                ),
            ),
        ),
        Node::let_bind("tile_block", Expr::WorkgroupId { axis: 0 }),
        Node::let_bind("tile_cols", Expr::u32(shape.n.div_ceil(tile.out_cols))),
        Node::let_bind(
            "tile_row_base",
            Expr::mul(
                Expr::div(Expr::var("tile_block"), Expr::var("tile_cols")),
                Expr::u32(tile.out_rows),
            ),
        ),
        Node::let_bind(
            "tile_col_base",
            Expr::mul(
                Expr::rem(Expr::var("tile_block"), Expr::var("tile_cols")),
                Expr::u32(tile.out_cols),
            ),
        ),
        Node::let_bind(
            names.lane_row,
            Expr::div(local.clone(), Expr::u32(tile.out_cols)),
        ),
        Node::let_bind(names.lane_col, Expr::rem(local, Expr::u32(tile.out_cols))),
        Node::let_bind(
            names.row,
            Expr::add(Expr::var("tile_row_base"), Expr::var(names.lane_row)),
        ),
        Node::let_bind(
            names.col,
            Expr::add(Expr::var("tile_col_base"), Expr::var(names.lane_col)),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_tile_coordinate_binding_is_single_shared_scaffold() {
        let nodes = bind_output_tile_coordinates(
            MatrixShape {
                m: 32,
                k: 16,
                n: 24,
            },
            TileShape {
                k_tile: 16,
                out_rows: 16,
                out_cols: 8,
                x_lanes: 32,
                y_lanes: 1,
                lanes: 32,
                a_values: 256,
                b_values: 128,
            },
            OutputTileCoordNames {
                lane_row: "lane_row",
                lane_col: "lane_col",
                row: "row0",
                col: "col",
            },
        );
        let debug = format!("{nodes:?}");
        for name in [
            "local",
            "tile_block",
            "tile_cols",
            "tile_row_base",
            "tile_col_base",
            "lane_row",
            "lane_col",
            "row0",
            "col",
        ] {
            assert!(
                debug.contains(name),
                "shared tile-coordinate scaffold must bind {name}"
            );
        }
    }
}
