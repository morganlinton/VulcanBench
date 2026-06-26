package cachettl

import (
	"fmt"
	"sync"
	"testing"
	"time"
)

const longTTL = time.Hour

func TestEmptyLen(t *testing.T) {
	c := New(2, longTTL)
	if c.Len() != 0 {
		t.Fatalf("Len = %d, want 0", c.Len())
	}
}

func TestBasicGetPut(t *testing.T) {
	c := New(2, longTTL)
	c.Put("a", 1)
	if v, ok := c.Get("a"); !ok || v != 1 {
		t.Fatalf("Get(a) = (%d,%v), want (1,true)", v, ok)
	}
	if _, ok := c.Get("missing"); ok {
		t.Fatal("Get(missing) should miss")
	}
}

func TestLRUEviction(t *testing.T) {
	c := New(2, longTTL)
	c.Put("a", 1)
	c.Put("b", 2)
	c.Put("c", 3) // evicts a (least recently used)
	if _, ok := c.Get("a"); ok {
		t.Fatal("a should have been evicted")
	}
	if v, ok := c.Get("b"); !ok || v != 2 {
		t.Fatalf("b should remain, got (%d,%v)", v, ok)
	}
	if v, ok := c.Get("c"); !ok || v != 3 {
		t.Fatalf("c should remain, got (%d,%v)", v, ok)
	}
}

func TestGetRefreshesRecency(t *testing.T) {
	c := New(2, longTTL)
	c.Put("a", 1)
	c.Put("b", 2)
	c.Get("a")    // a is now most recently used
	c.Put("c", 3) // should evict b, not a
	if _, ok := c.Get("b"); ok {
		t.Fatal("b should have been evicted (a was refreshed by Get)")
	}
	if _, ok := c.Get("a"); !ok {
		t.Fatal("a should remain")
	}
}

func TestUpdateRefreshesRecency(t *testing.T) {
	c := New(2, longTTL)
	c.Put("a", 1)
	c.Put("b", 2)
	c.Put("a", 10) // update makes a most recently used
	c.Put("c", 3)  // should evict b
	if _, ok := c.Get("b"); ok {
		t.Fatal("b should have been evicted")
	}
	if v, ok := c.Get("a"); !ok || v != 10 {
		t.Fatalf("a should remain with updated value, got (%d,%v)", v, ok)
	}
}

func TestTTLExpiry(t *testing.T) {
	c := New(4, 50*time.Millisecond)
	c.Put("a", 1)
	if _, ok := c.Get("a"); !ok {
		t.Fatal("a should be live immediately after Put")
	}
	time.Sleep(80 * time.Millisecond)
	if _, ok := c.Get("a"); ok {
		t.Fatal("a should have expired")
	}
	if c.Len() != 0 {
		t.Fatalf("Len = %d after expiry, want 0", c.Len())
	}
}

func TestUpdateRefreshesTTL(t *testing.T) {
	c := New(4, 90*time.Millisecond)
	c.Put("a", 1)
	time.Sleep(60 * time.Millisecond)
	c.Put("a", 2) // refreshes the TTL
	time.Sleep(60 * time.Millisecond)
	if v, ok := c.Get("a"); !ok || v != 2 {
		t.Fatalf("a should still be live (TTL refreshed), got (%d,%v)", v, ok)
	}
}

func TestConcurrentAccessIsRaceFree(t *testing.T) {
	const cap = 50
	const keys = 200
	c := New(cap, longTTL)
	var wg sync.WaitGroup
	for i := 0; i < keys; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			k := fmt.Sprintf("k%d", i)
			c.Put(k, i)
			c.Get(k)
			c.Get(fmt.Sprintf("k%d", (i+1)%keys))
		}(i)
	}
	wg.Wait()
	if got := c.Len(); got != cap {
		t.Fatalf("Len = %d after %d distinct puts, want capacity %d", got, keys, cap)
	}
}
