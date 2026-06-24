// Package pmap applies a function over a slice concurrently, with a bound on
// the number of in-flight goroutines, preserving input order.
package pmap

import (
	"context"
	"sync"
)

// Map applies f to every element of in using at most `limit` concurrent
// goroutines and returns the results in the same order as in. The per-element
// ctx is forwarded to f so callers can cancel. If one or more calls return an
// error, Map returns the error belonging to the lowest input index. A limit
// below 1 is treated as 1.
//
// NOTE: this implementation has bugs — see issue.md.
func Map(ctx context.Context, in []int, limit int, f func(context.Context, int) (int, error)) ([]int, error) {
	if limit < 1 {
		limit = 1
	}
	var results []int
	var firstErr error
	sem := make(chan struct{}, limit)
	var wg sync.WaitGroup
	for _, x := range in {
		sem <- struct{}{}
		wg.Add(1)
		go func(x int) {
			defer wg.Done()
			defer func() { <-sem }()
			v, err := f(ctx, x)
			if err != nil {
				if firstErr == nil {
					firstErr = err
				}
				return
			}
			results = append(results, v)
		}(x)
	}
	wg.Wait()
	return results, firstErr
}
