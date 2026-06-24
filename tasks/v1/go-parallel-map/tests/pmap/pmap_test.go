package pmap

import (
	"context"
	"errors"
	"reflect"
	"sync/atomic"
	"testing"
	"time"
)

// delayed returns an f that finishes after x*unit so that completion order
// differs from input order. It returns x*10 on success.
func delayed(unit time.Duration) func(context.Context, int) (int, error) {
	return func(ctx context.Context, x int) (int, error) {
		select {
		case <-time.After(time.Duration(x) * unit):
			return x * 10, nil
		case <-ctx.Done():
			return 0, ctx.Err()
		}
	}
}

// --- pass_to_pass: no concurrency exercised, the buggy version already passes ---

func TestEmpty(t *testing.T) {
	got, err := Map(context.Background(), []int{}, 2, delayed(time.Millisecond))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(got) != 0 {
		t.Fatalf("Map(empty) = %v, want empty", got)
	}
}

func TestSingle(t *testing.T) {
	got, err := Map(context.Background(), []int{7}, 1, delayed(time.Millisecond))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !reflect.DeepEqual(got, []int{70}) {
		t.Fatalf("Map = %v, want [70]", got)
	}
}

// --- fail_to_pass ---

func TestOrderPreserved(t *testing.T) {
	// Later inputs finish first; results must still follow input order.
	in := []int{5, 1, 3, 2, 4}
	got, err := Map(context.Background(), in, len(in), delayed(15*time.Millisecond))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	want := []int{50, 10, 30, 20, 40}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Map = %v, want %v (results must follow input order)", got, want)
	}
}

func TestFirstErrorByInputIndex(t *testing.T) {
	errNine := errors.New("nine failed")
	errOne := errors.New("one failed")
	// index 0 (value 9) errors but finishes last; index 2 (value 1) errors and
	// finishes first. Map must return the error at the lowest input index (9),
	// not the first one to complete (1).
	f := func(ctx context.Context, x int) (int, error) {
		select {
		case <-time.After(time.Duration(x) * 15 * time.Millisecond):
		case <-ctx.Done():
			return 0, ctx.Err()
		}
		switch x {
		case 9:
			return 0, errNine
		case 1:
			return 0, errOne
		}
		return x, nil
	}
	_, err := Map(context.Background(), []int{9, 6, 1}, 3, f)
	if !errors.Is(err, errNine) {
		t.Fatalf("err = %v, want lowest-input-index error %v", err, errNine)
	}
}

func TestConcurrencyLimitRespected(t *testing.T) {
	var inflight, maxSeen int32
	f := func(ctx context.Context, x int) (int, error) {
		cur := atomic.AddInt32(&inflight, 1)
		for {
			old := atomic.LoadInt32(&maxSeen)
			if cur <= old || atomic.CompareAndSwapInt32(&maxSeen, old, cur) {
				break
			}
		}
		time.Sleep(20 * time.Millisecond)
		atomic.AddInt32(&inflight, -1)
		return x, nil
	}
	in := make([]int, 12)
	for i := range in {
		in[i] = i
	}
	if _, err := Map(context.Background(), in, 3, f); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if maxSeen > 3 {
		t.Fatalf("observed %d concurrent calls, limit was 3", maxSeen)
	}
}
