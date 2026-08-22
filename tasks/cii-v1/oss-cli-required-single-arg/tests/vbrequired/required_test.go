// Hidden fail-to-pass tests: single-value arguments marked Required.
// Lives in its own test-only package so the pass-to-pass guards compile at the
// base commit even while these tests reference the new API.
package vbrequired

import (
	"context"
	"strings"
	"testing"

	cli "github.com/urfave/cli/v3"
)

func TestVBSingleRequiredArgMissing(t *testing.T) {
	var got string
	cmd := &cli.Command{
		Name: "app",
		Arguments: []cli.Argument{
			&cli.StringArg{Name: "id", Destination: &got, Required: true},
		},
		Action: func(ctx context.Context, c *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err == nil {
		t.Fatal("running without the required argument must return an error")
	}
	if err := cmd.Run(context.Background(), []string{"app", "abc123"}); err != nil {
		t.Fatalf("running with the argument must succeed, got %v", err)
	}
	if got != "abc123" {
		t.Fatalf("destination = %q, want %q", got, "abc123")
	}
}

func TestVBRequiredArgDefaultDoesNotSatisfy(t *testing.T) {
	// A default Value does not satisfy Required, same as with flags.
	var got string
	cmd := &cli.Command{
		Name: "app",
		Arguments: []cli.Argument{
			&cli.StringArg{Name: "id", Value: "fallback", Destination: &got, Required: true},
		},
		Action: func(ctx context.Context, c *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err == nil {
		t.Fatal("default value must not satisfy a required argument")
	}
}

func TestVBChainedRequiredArgs(t *testing.T) {
	var a, b string
	cmd := &cli.Command{
		Name: "app",
		Arguments: []cli.Argument{
			&cli.StringArg{Name: "src", Destination: &a, Required: true},
			&cli.StringArg{Name: "dst", Destination: &b, Required: true},
		},
		Action: func(ctx context.Context, c *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app", "only-one"}); err == nil {
		t.Fatal("missing second required argument must return an error")
	}
	if err := cmd.Run(context.Background(), []string{"app", "x", "y"}); err != nil {
		t.Fatalf("both arguments given must succeed, got %v", err)
	}
	if a != "x" || b != "y" {
		t.Fatalf("destinations = %q, %q; want x, y", a, b)
	}
}

func TestVBUsageRendering(t *testing.T) {
	// A required single arg renders bare; an optional one renders bracketed,
	// matching what the plural {Type}Args already do.
	req := &cli.StringArg{Name: "id", Required: true}
	opt := &cli.StringArg{Name: "note"}
	if u := req.Usage(); strings.Contains(u, "[") {
		t.Fatalf("required arg usage %q must not be bracketed", u)
	}
	if u := opt.Usage(); !strings.Contains(u, "[") || !strings.Contains(u, "note") {
		t.Fatalf("optional arg usage %q must be bracketed", u)
	}
}
