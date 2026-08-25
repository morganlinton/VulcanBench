//! MMA-oriented tiled matmul body for full F16 M16N8-aligned tiles.

use vyre::ir::{DataType, Expr, Node};

use super::mma_fragment::matmul_mma_fragment;
use super::shape::{MatrixShape, TileShape};
use super::tile_coords::{bind_output_tile_coordinates, OutputTileCoordNames};

pub(super) fn cooperative_matmul_body_mma(
    a: &str,
    b: &str,
    bias: Option<&str>,
    out: &str,
    shape: MatrixShape,
    tile: TileShape,
    _dtype: DataType,
    _a_tile_name: &str,
    _b_tile_name: &str,
) -> Vec<Node> {
    let local = Expr::var("local");
    let col = Expr::var("col");
    let row0 = Expr::var("row0");
    let row1 = Expr::var("row1");
    let row2 = Expr::var("row2");
    let row3 = Expr::var("row3");
    let col_in_bounds = Expr::lt(col.clone(), Expr::u32(shape.n));
    let row0_in_bounds = Expr::and(
        Expr::lt(row0.clone(), Expr::u32(shape.m)),
        col_in_bounds.clone(),
    );
    let row1_in_bounds = Expr::and(
        Expr::lt(row1.clone(), Expr::u32(shape.m)),
        col_in_bounds.clone(),
    );
    let row2_in_bounds = Expr::and(
        Expr::lt(row2.clone(), Expr::u32(shape.m)),
        col_in_bounds.clone(),
    );
    let row3_in_bounds = Expr::and(
        Expr::lt(row3.clone(), Expr::u32(shape.m)),
        col_in_bounds.clone(),
    );

    let mut body = bind_output_tile_coordinates(
        shape,
        tile,
        OutputTileCoordNames {
            lane_row: "lane_row",
            lane_col: "lane_col",
            row: "row0",
            col: "col",
        },
    );
    body.extend([
        Node::let_bind("row1", Expr::add(Expr::var("row0"), Expr::u32(4))),
        Node::let_bind("row2", Expr::add(Expr::var("row0"), Expr::u32(8))),
        Node::let_bind("row3", Expr::add(Expr::var("row0"), Expr::u32(12))),
        Node::let_bind("acc0", Expr::u32(0)),
        Node::let_bind("acc1", Expr::u32(0)),
        Node::let_bind("acc2", Expr::u32(0)),
        Node::let_bind("acc3", Expr::u32(0)),
    ]);
    if let Some(bias) = bias {
        body.push(Node::if_then(
            col_in_bounds.clone(),
            vec![
                Node::assign("acc0", Expr::load(bias, col.clone())),
                Node::assign("acc1", Expr::load(bias, col.clone())),
                Node::assign("acc2", Expr::load(bias, col.clone())),
                Node::assign("acc3", Expr::load(bias, col.clone())),
            ],
        ));
    }
    body.push(Node::loop_for(
        "kk",
        Expr::u32(0),
        Expr::u32(shape.k),
        vec![Node::let_bind(
            "b_val",
            Expr::load(
                b,
                Expr::add(Expr::mul(Expr::var("kk"), Expr::u32(shape.n)), col.clone()),
            ),
        )]
        .into_iter()
        .chain(matmul_mma_fragment(
            Expr::load(
                a,
                Expr::add(Expr::mul(row0.clone(), Expr::u32(shape.k)), Expr::var("kk")),
            ),
            Expr::load(
                a,
                Expr::add(Expr::mul(row1.clone(), Expr::u32(shape.k)), Expr::var("kk")),
            ),
            Expr::load(
                a,
                Expr::add(Expr::mul(row2.clone(), Expr::u32(shape.k)), Expr::var("kk")),
            ),
            Expr::load(
                a,
                Expr::add(Expr::mul(row3.clone(), Expr::u32(shape.k)), Expr::var("kk")),
            ),
            Expr::var("b_val"),
            Expr::var("b_val"),
            Expr::var("acc0"),
            Expr::var("acc1"),
            Expr::var("acc2"),
            Expr::var("acc3"),
        ))
        .chain([
            Node::if_then(
                row0_in_bounds.clone(),
                vec![Node::assign("acc0", Expr::var("mma_c0"))],
            ),
            Node::if_then(
                row1_in_bounds.clone(),
                vec![Node::assign("acc1", Expr::var("mma_c1"))],
            ),
            Node::if_then(
                row2_in_bounds.clone(),
                vec![Node::assign("acc2", Expr::var("mma_c2"))],
            ),
            Node::if_then(
                row3_in_bounds.clone(),
                vec![Node::assign("acc3", Expr::var("mma_c3"))],
            ),
        ])
        .collect(),
    ));
    body.extend([
        Node::if_then(
            row0_in_bounds,
            vec![Node::Store {
                buffer: out.into(),
                index: Expr::add(Expr::mul(row0, Expr::u32(shape.n)), col.clone()),
                value: Expr::var("acc0"),
            }],
        ),
        Node::if_then(
            row1_in_bounds,
            vec![Node::Store {
                buffer: out.into(),
                index: Expr::add(Expr::mul(row1, Expr::u32(shape.n)), col.clone()),
                value: Expr::var("acc1"),
            }],
        ),
        Node::if_then(
            row2_in_bounds,
            vec![Node::Store {
                buffer: out.into(),
                index: Expr::add(Expr::mul(row2, Expr::u32(shape.n)), col.clone()),
                value: Expr::var("acc2"),
            }],
        ),
        Node::if_then(
            row3_in_bounds,
            vec![Node::Store {
                buffer: out.into(),
                index: Expr::add(Expr::mul(row3, Expr::u32(shape.n)), col),
                value: Expr::var("acc3"),
            }],
        ),
    ]);
    body
}
