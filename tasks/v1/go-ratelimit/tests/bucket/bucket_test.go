package bucket

import (
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

type clock struct {
	mu sync.Mutex
	t  time.Time
}

func (c *clock) now() time.Time      { c.mu.Lock(); defer c.mu.Unlock(); return c.t }
func (c *clock) advance(d time.Duration) { c.mu.Lock(); defer c.mu.Unlock(); c.t = c.t.Add(d) }
func newClock() *clock               { return &clock{t: time.Unix(0, 0)} }

func TestAllowUntilEmpty(t *testing.T) {
	c := newClock()
	b := New(3, 1, c.now)
	for i := 0; i < 3; i++ {
		if !b.Allow() {
			t.Fatalf("expected Allow #%d to succeed", i)
		}
	}
	if b.Allow() {
		t.Fatal("expected Allow to fail once the bucket is empty")
	}
}

func TestRefillIsGradual(t *testing.T) {
	c := newClock()
	b := New(5, 1, c.now) // 1 token/sec
	for i := 0; i < 5; i++ {
		b.Allow()
	}
	if b.Allow() {
		t.Fatal("bucket should be empty after draining")
	}
	c.advance(3 * time.Second) // +3 tokens
	for i := 0; i < 3; i++ {
		if !b.Allow() {
			t.Fatalf("expected refilled Allow #%d to succeed", i)
		}
	}
	if b.Allow() {
		t.Fatal("bucket should be empty again")
	}
}

func TestCapacityIsClamped(t *testing.T) {
	c := newClock()
	b := New(5, 10, c.now)
	for i := 0; i < 5; i++ {
		b.Allow() // drain to zero
	}
	c.advance(100 * time.Second) // would add 1000 tokens if unclamped
	n := 0
	for b.Allow() {
		n++
		if n > 1000 {
			break
		}
	}
	if n != 5 {
		t.Fatalf("refill must clamp to capacity 5, got %d tokens", n)
	}
}

func TestConcurrentAllowGrantsExactlyCapacity(t *testing.T) {
	c := newClock() // fixed clock: no refill during the test
	b := New(100, 1, c.now)
	var granted int64
	var wg sync.WaitGroup
	start := make(chan struct{})
	for i := 0; i < 500; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			<-start
			if b.Allow() {
				atomic.AddInt64(&granted, 1)
			}
		}()
	}
	close(start)
	wg.Wait()
	if granted != 100 {
		t.Fatalf("exactly capacity (100) tokens should be granted, got %d", granted)
	}
}
