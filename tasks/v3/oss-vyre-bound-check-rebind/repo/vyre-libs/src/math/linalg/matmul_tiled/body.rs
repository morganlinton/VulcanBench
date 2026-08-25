//! Cooperative inner kernel body shared by the plain and bias-fused
//! tiled matmul variants.

use vyre::ir::{Expr, Node};

use super::shape::{in_output_bounds, MatrixShape, TileShape};
use super::tile_coords::{bind_output_tile_coordinates, OutputTileCoordNames};

pub(crate) fn cooperative_matmul_body(
    a: &str,
    b: &str,
    bias: Option<&str>,
    out: &str,
    shape: MatrixShape,
    tile: TileShape,
) -> Vec<Node> {
    let tile_count = shape.k.div_ceil(tile.k_tile);
    let load_passes = tile.a_values.max(tile.b_values).div_ceil(tile.lanes).max(1);
    let local = Expr::var("local");
    let row = Expr::var("row");
    let col = Expr::var("col");
    let in_bounds = in_output_bounds(row.clone(), col.clone(), shape);
    let out_index = Expr::add(Expr::mul(row.clone(), Expr::u32(shape.n)), col.clone());

    let mut body = bind_output_tile_coordinates(
        shape,
        tile,
        OutputTileCoordNames {
            lane_row: "local_row",
            lane_col: "local_col",
            row: "row",
            col: "col",
        },
    );
    body.push(Node::let_bind("acc", Expr::u32(0)));
    if let Some(bias) = bias {
        body.push(Node::if_then(
            in_bounds.clone(),
            vec![Node::assign("acc", Expr::load(bias, col.clone()))],
        ));
    }
    body.push(Node::loop_for(
        "tile_idx",
        Expr::u32(0),
        Expr::u32(tile_count),
        vec![
            Node::let_bind(
                "k_base",
                Expr::mul(Expr::var("tile_idx"), Expr::u32(tile.k_tile)),
            ),
            Node::loop_for(
                "load_pass",
                Expr::u32(0),
                Expr::u32(load_passes),
                vec![
                    Node::let_bind(
                        "a_linear",
                        Expr::add(
                            local.clone(),
                            Expr::mul(Expr::var("load_pass"), Expr::u32(tile.lanes)),
                        ),
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("a_linear"), Expr::u32(tile.a_values)),
                        vec![
                            Node::let_bind(
                                "a_local_row",
                                Expr::div(Expr::var("a_linear"), Expr::u32(tile.k_tile)),
                            ),
                            Node::let_bind(
                                "a_local_k",
                                Expr::rem(Expr::var("a_linear"), Expr::u32(tile.k_tile)),
                            ),
                            Node::let_bind(
                                "a_row",
                                Expr::add(Expr::var("tile_row_base"), Expr::var("a_local_row")),
                            ),
                            Node::let_bind(
                                "a_k",
                                Expr::add(Expr::var("k_base"), Expr::var("a_local_k")),
                            ),
                            Node::Store {
                                buffer: "matmul_a_tile".into(),
                                index: Expr::var("a_linear"),
                                value: Expr::u32(0),
                            },
                            Node::if_then(
                                Expr::and(
                                    Expr::lt(Expr::var("a_row"), Expr::u32(shape.m)),
                                    Expr::lt(Expr::var("a_k"), Expr::u32(shape.k)),
                                ),
                                vec![Node::Store {
                                    buffer: "matmul_a_tile".into(),
                                    index: Expr::var("a_linear"),
                                    value: Expr::load(
                                        a,
                                        Expr::add(
                                            Expr::mul(Expr::var("a_row"), Expr::u32(shape.k)),
                                            Expr::var("a_k"),
                                        ),
                                    ),
                                }],
                            ),
                        ],
                    ),
                    Node::let_bind(
                        "b_linear",
                        Expr::add(
                            local.clone(),
                            Expr::mul(Expr::var("load_pass"), Expr::u32(tile.lanes)),
                        ),
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("b_linear"), Expr::u32(tile.b_values)),
                        vec![
                            Node::let_bind(
                                "b_local_k",
                                Expr::div(Expr::var("b_linear"), Expr::u32(tile.out_cols)),
                            ),
                            Node::let_bind(
                                "b_local_col",
                                Expr::rem(Expr::var("b_linear"), Expr::u32(tile.out_cols)),
                            ),
                            Node::let_bind(
                                "b_k",
                                Expr::add(Expr::var("k_base"), Expr::var("b_local_k")),
                            ),
                            Node::let_bind(
                                "b_col",
                                Expr::add(Expr::var("tile_col_base"), Expr::var("b_local_col")),
                            ),
                            Node::Store {
                                buffer: "matmul_b_tile".into(),
                                index: Expr::var("b_linear"),
                                value: Expr::u32(0),
                            },
                            Node::if_then(
                                Expr::and(
                                    Expr::lt(Expr::var("b_k"), Expr::u32(shape.k)),
                                    Expr::lt(Expr::var("b_col"), Expr::u32(shape.n)),
                                ),
                                vec![Node::Store {
                                    buffer: "matmul_b_tile".into(),
                                    index: Expr::var("b_linear"),
                                    value: Expr::load(
                                        b,
                                        Expr::add(
                                            Expr::mul(Expr::var("b_k"), Expr::u32(shape.n)),
                                            Expr::var("b_col"),
                                        ),
                                    ),
                                }],
                            ),
                        ],
                    ),
                ],
            ),
            Node::barrier(),
            Node::loop_for(
                "tile_k",
                Expr::u32(0),
                Expr::u32(tile.k_tile),
                vec![Node::if_then(
                    in_bounds.clone(),
                    vec![Node::assign(
                        "acc",
                        Expr::add(
                            Expr::var("acc"),
                            Expr::mul(
                                Expr::load(
                                    "matmul_a_tile",
                                    Expr::add(
                                        Expr::mul(Expr::var("local_row"), Expr::u32(tile.k_tile)),
                                        Expr::var("tile_k"),
                                    ),
                                ),
                                Expr::load(
                                    "matmul_b_tile",
                                    Expr::add(
                                        Expr::mul(Expr::var("tile_k"), Expr::u32(tile.out_cols)),
                                        Expr::var("local_col"),
                                    ),
                                ),
                            ),
                        ),
                    )],
                )],
            ),
            Node::barrier(),
        ],
    ));
    body.push(Node::if_then(
        in_bounds,
        vec![Node::Store {
            buffer: out.into(),
            index: out_index,
            value: Expr::var("acc"),
        }],
    ));
    body
}
