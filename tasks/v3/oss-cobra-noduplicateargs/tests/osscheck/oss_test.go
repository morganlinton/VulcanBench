package osscheck

import (
	"strings"
	"testing"

	"github.com/spf13/cobra"
)

// Net-new validator: cobra.NoDuplicateArgs(cmd, args) returns an error when the
// positional args contain a duplicate. At the base commit this symbol does not
// exist, so this package fails to compile (all tests fail).

func TestNoDuplicateArgsRejectsDuplicates(t *testing.T) {
	c := &cobra.Command{Use: "root"}
	if err := cobra.NoDuplicateArgs(c, []string{"a", "b", "a"}); err == nil {
		t.Fatalf("expected error for duplicate args, got nil")
	}
}

func TestNoDuplicateArgsMessageFormat(t *testing.T) {
	c := &cobra.Command{Use: "root"}
	err := cobra.NoDuplicateArgs(c, []string{"x", "x"})
	if err == nil || !strings.Contains(err.Error(), `duplicate argument "x"`) {
		t.Fatalf("expected duplicate-argument message, got %v", err)
	}
}

func TestNoDuplicateArgsAllowsUnique(t *testing.T) {
	c := &cobra.Command{Use: "root"}
	if err := cobra.NoDuplicateArgs(c, []string{"a", "b", "c"}); err != nil {
		t.Fatalf("expected nil for unique args, got %v", err)
	}
}
