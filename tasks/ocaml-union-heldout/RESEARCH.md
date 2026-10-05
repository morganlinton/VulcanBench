# Open-union workload rationale

The full bin_prot v0.17.0 checkout at
2cd58a8cd74b5fa1f28d63cbc4deaf6665b82525 is the source substrate. The native
Type_class writer/read interface, Bigarray-backed Common.buf, Utils.bin_dump
and Utils.bin_read_stream are inspected in the supplied source. Their upstream
provenance is https://github.com/janestreet/bin_prot/tree/2cd58a8cd74b5fa1f28d63cbc4deaf6665b82525.
Jane Street relevance is inferred from this public library; no internal access
or endorsement is claimed. All original licenses/notices remain present.

The new Union module tests existential case selection across different payload
types, ambiguous projection, open forwarding and composition with the native
reader API. The dependency intentionally supplies the completed earlier Tagged
reference, including its short-stream correction. No earlier hidden tests,
reference diff, calibration notes or outcome receipts enter the public repo.
The new Union reference/checks are separately authored before target attempts.
Authoring nevertheless uses development feedback, so this is not decontaminated
or independent external confirmation. Repository scale includes upstream
generated fixtures and is not treated as a proxy for difficulty.

The upstream test library cannot build without the uninstalled float_array
package. Preflight builds the actual bin_prot library, fixture-checksum alias
and supplied public clients at the original pinned environment. It does not
claim every upstream inline test executes. The default budgets and serial
native settings remain unchanged. No resources/network isolation are asserted
for target execution; validation separately denies network for its process tree.
