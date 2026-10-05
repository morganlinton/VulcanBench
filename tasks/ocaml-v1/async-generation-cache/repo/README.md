# Async generation cache

An original service-library fixture built on Core and Async_kernel v0.17.
Policy and Request implement the existing configuration and request lifecycle.
Client demonstrates a batch caller. Alarm and Cache need implementation.

Use the supplied simulated Time_source rather than wall-clock delays. Installed
dependency interfaces are available under /opt/opam/5.2.1/lib/async_kernel,
including time_source_intf.ml and monitor.mli. Run dune runtest for public tests.
