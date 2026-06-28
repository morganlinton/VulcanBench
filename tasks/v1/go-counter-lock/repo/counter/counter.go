// Package counter provides a goroutine-safe counter.
package counter

import "sync"

// SafeCounter is safe for concurrent use; every method holds the lock.
type SafeCounter struct {
	mu     sync.Mutex
	counts map[string]int
}

// New returns an empty SafeCounter.
func New() *SafeCounter {
	return &SafeCounter{counts: make(map[string]int)}
}

// Inc increments the count for key by 1.
func (c *SafeCounter) Inc(key string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.counts[key]++
}

// Get returns the current count for key.
func (c *SafeCounter) Get(key string) int {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.counts[key]
}
