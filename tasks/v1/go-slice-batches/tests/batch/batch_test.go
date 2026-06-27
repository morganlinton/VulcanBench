package batch

import (
	"reflect"
	"testing"
)

func TestBatchesContents(t *testing.T) {
	got := Batches([]int{1, 2, 3, 4, 5}, 2)
	want := [][]int{{1, 2}, {3, 4}, {5}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Batches = %v, want %v", got, want)
	}
}

func TestSizeBelowOneIsTreatedAsOne(t *testing.T) {
	got := Batches([]int{1, 2, 3}, 0)
	want := [][]int{{1}, {2}, {3}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Batches size 0 = %v, want %v", got, want)
	}
}

func TestAppendingToABatchDoesNotCorruptTheNext(t *testing.T) {
	b := Batches([]int{1, 2, 3, 4}, 2)
	_ = append(b[0], 99) // must not write into b[1]'s storage
	if !reflect.DeepEqual(b[1], []int{3, 4}) {
		t.Fatalf("b[1] = %v, want [3 4] (appending to b[0] leaked into it)", b[1])
	}
}

func TestBatchesDoNotAliasTheInput(t *testing.T) {
	in := []int{1, 2, 3, 4}
	b := Batches(in, 2)
	_ = append(b[0], 99) // must not write into the caller's slice
	if !reflect.DeepEqual(in, []int{1, 2, 3, 4}) {
		t.Fatalf("input mutated to %v, want [1 2 3 4]", in)
	}
}

func TestEachBatchHasNoSpareCapacity(t *testing.T) {
	for _, b := range Batches([]int{1, 2, 3, 4, 5}, 2) {
		if cap(b) != len(b) {
			t.Fatalf("batch %v has cap %d != len %d (it shares a backing array)", b, cap(b), len(b))
		}
	}
}
