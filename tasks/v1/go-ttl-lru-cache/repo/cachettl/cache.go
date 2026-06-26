// Package cachettl is a fixed-capacity, thread-safe LRU cache with per-entry TTL.
package cachettl

import "time"

// Cache is a thread-safe LRU cache holding at most `capacity` live entries.
// Entries expire `ttl` after they were last written.
//
// NOTE: this is an unimplemented skeleton — see issue.md.
type Cache struct {
	capacity int
	ttl      time.Duration
}

// New returns a Cache holding at most capacity entries, each living for ttl.
func New(capacity int, ttl time.Duration) *Cache {
	return &Cache{capacity: capacity, ttl: ttl}
}

// Get returns the value for key and true if present and not expired; a fresh
// read makes key the most recently used. Otherwise it returns (0, false).
func (c *Cache) Get(key string) (int, bool) {
	return 0, false
}

// Put inserts or updates key=value. If that grows the cache past capacity, the
// least recently used live entry is evicted.
func (c *Cache) Put(key string, value int) {
}

// Len returns the number of live (non-expired) entries.
func (c *Cache) Len() int {
	return 0
}
