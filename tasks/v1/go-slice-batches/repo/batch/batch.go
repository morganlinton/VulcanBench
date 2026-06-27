// Package batch splits a slice into consecutive batches.
//
// NOTE: Batches has a bug — each returned batch is a sub-slice of the input's
// backing array, so the batches alias each other and the input. Appending to one
// batch can silently overwrite the next batch (and mutate the caller's slice).
// See issue.md.
package batch

// Batches splits items into consecutive batches of at most size elements. A size
// below 1 is treated as 1. Each batch must be independent of the others and of
// the input: appending to or growing one batch must not disturb anything else.
func Batches(items []int, size int) [][]int {
	if size < 1 {
		size = 1
	}
	var out [][]int
	for i := 0; i < len(items); i += size {
		end := i + size
		if end > len(items) {
			end = len(items)
		}
		out = append(out, items[i:end])
	}
	return out
}
