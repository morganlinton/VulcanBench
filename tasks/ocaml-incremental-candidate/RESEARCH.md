# Public-source rationale

Jane Street describes Incremental as useful for financial calculations and
responsive, data-rich web interfaces in
[One more talk, two more videos](https://blog.janestreet.com/one-more-talk-two-more-videos/).
Its [Seven Implementations of Incremental](https://www.janestreet.com/tech-talks/seven-implementations-of-incremental/)
talk explains self-adjusting computations and applications to compute engines
and user interfaces. These public examples motivate evaluating engineers'
ability to change the propagation engine while preserving its semantics.

The task is our original proposed extension. Neither source claims Jane Street
needs this particular API or endorses this benchmark. A bounded propagation
slice could help an application schedule work cooperatively. That usefulness
is our engineering inference, not an internal Jane Street requirement. A node
quota is not a latency guarantee because one callback can take arbitrarily long.

The pinned [public interface](https://raw.githubusercontent.com/janestreet/incremental/v0.17.0/src/incremental_intf.ml)
already offers Expert.do_one_step_of_stabilize. The new task deliberately
requires an actual recomputation bound rather than wrapping the existing step:
the [scheduler](https://raw.githubusercontent.com/janestreet/incremental/v0.17.0/src/state.ml)
may recursively recompute parents without additional heap pops. It also manages
dynamic scopes, deferred variable writes, observer linking and publication.
Their interaction provides meaningful repository engineering and OCaml type
mastery, including mutually recursive state records and generative witnesses.

This full checkout retains public tests and MIT licensing. It is not a known
upstream bug reproduction and makes no decontamination or model-cutoff claim.
