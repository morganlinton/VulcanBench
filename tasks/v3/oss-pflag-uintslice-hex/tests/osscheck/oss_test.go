package osscheck

import (
	"testing"

	"github.com/spf13/pflag"
)

func parse(t *testing.T, val string) []uint {
	t.Helper()
	fs := pflag.NewFlagSet("t", pflag.ContinueOnError)
	fs.UintSlice("nums", nil, "")
	if err := fs.Parse([]string{"--nums=" + val}); err != nil {
		t.Fatalf("parse %q failed: %v", val, err)
	}
	got, err := fs.GetUintSlice("nums")
	if err != nil {
		t.Fatalf("GetUintSlice failed: %v", err)
	}
	return got
}

func eq(t *testing.T, got []uint, want []uint) {
	t.Helper()
	if len(got) != len(want) {
		t.Fatalf("len mismatch: got %v want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("got %v want %v", got, want)
		}
	}
}

// fail_to_pass: base uses ParseUint base 10 and rejects 0x/0o/0b prefixes.
func TestUintSliceHex(t *testing.T)   { eq(t, parse(t, "0xff"), []uint{255}) }
func TestUintSliceMixedRadix(t *testing.T) {
	eq(t, parse(t, "0x10,0o17,0b101"), []uint{16, 15, 5})
}
func TestUintSliceHexLarge(t *testing.T) { eq(t, parse(t, "0xdead"), []uint{57005}) }

// pass_to_pass: decimal parsing is unchanged (works at base and after the fix).
func TestUintSliceDecimalUnaffected(t *testing.T) {
	eq(t, parse(t, "1,2,3"), []uint{1, 2, 3})
	eq(t, parse(t, "255"), []uint{255})
}
