// Package cache is a concurrency-safe int cache.
package cache

import "sync"

// Cache maps string keys to ints and is safe for concurrent use.
type Cache struct {
	mu sync.Mutex
	m  map[string]int
}

// New returns an empty Cache.
func New() *Cache {
	return &Cache{m: make(map[string]int)}
}

// Get returns the value for key and whether it was present.
func (c *Cache) Get(key string) (int, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	v, ok := c.m[key]
	return v, ok
}

// Set stores v under key.
func (c *Cache) Set(key string, v int) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.m[key] = v
}
