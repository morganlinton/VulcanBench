// Package bus is a tiny concurrency-safe subscriber list.
package bus

import "sync"

// Bus holds subscriber callbacks and is safe for concurrent use.
type Bus struct {
	mu   sync.Mutex
	subs []func(string)
}

// Subscribe registers fn to receive published messages.
func (b *Bus) Subscribe(fn func(string)) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.subs = append(b.subs, fn)
}
