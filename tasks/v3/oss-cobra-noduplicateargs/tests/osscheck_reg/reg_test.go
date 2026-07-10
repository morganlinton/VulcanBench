package osscheck_reg

import (
	"testing"

	"github.com/spf13/cobra"
)

// pass_to_pass regression guard over pre-existing arg validators. Does NOT
// reference the net-new NoDuplicateArgs, so it compiles and passes at the base
// commit as well as after the fix.

func TestExistingArgValidatorsUnaffected(t *testing.T) {
	c := &cobra.Command{Use: "root"}
	if err := cobra.ArbitraryArgs(c, []string{"a", "a"}); err != nil {
		t.Fatalf("ArbitraryArgs should accept anything, got %v", err)
	}
	if err := cobra.NoArgs(c, []string{}); err != nil {
		t.Fatalf("NoArgs should accept empty, got %v", err)
	}
	if err := cobra.NoArgs(c, []string{"x"}); err == nil {
		t.Fatalf("NoArgs should reject a positional arg")
	}
	if err := cobra.ExactArgs(2)(c, []string{"a", "b"}); err != nil {
		t.Fatalf("ExactArgs(2) should accept two args, got %v", err)
	}
}
