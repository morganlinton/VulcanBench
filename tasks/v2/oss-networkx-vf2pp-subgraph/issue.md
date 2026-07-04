The VF2++ isomorphism module only supports whole-graph isomorphism. Add subgraph-isomorphism and monomorphism support.

The module `networkx.algorithms.isomorphism.vf2pp` currently exposes only
`vf2pp_isomorphism`, `vf2pp_is_isomorphic`, and `vf2pp_all_isomorphisms`, which
test whether two graphs are isomorphic as wholes. Extend the VF2++ matcher to
also answer subgraph-isomorphism and monomorphism queries, adding:

- `vf2pp_subgraph_isomorphism`, `vf2pp_subgraph_is_isomorphic`,
  `vf2pp_all_subgraph_isomorphisms`
- `vf2pp_monomorphism`, `vf2pp_is_monomorphic`, `vf2pp_all_monomorphisms`

Each takes a host graph `FG` and a pattern graph `SG` and looks for the pattern
inside the host. The two query types differ in their edge semantics:

- **Subgraph isomorphism is node-induced.** A match maps the pattern's nodes to
  a subset of host nodes such that the host subgraph induced on those nodes has
  exactly the pattern's edges (no more, no fewer). For example, a path P3 has no
  induced copy inside a complete graph K4, because any three K4 nodes induce a
  triangle.
- **Monomorphism is an edge-preserving injection.** A match maps the pattern's
  nodes injectively into host nodes so that every pattern edge maps to a host
  edge; extra edges among the mapped host nodes are allowed. So P3 does have a
  monomorphism into a triangle, even though it has no induced copy.

The `..._is_...` predicates return a bool, the singular functions return one
mapping (or `None`), and the `..._all_...` functions yield every distinct
mapping. Optional `node_label` / `default_label` arguments constrain matches to
equal labels, as in the existing isomorphism functions. The existing whole-graph
isomorphism behavior must be unchanged.
