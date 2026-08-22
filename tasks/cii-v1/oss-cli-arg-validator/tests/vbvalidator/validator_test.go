// Hidden fail-to-pass tests: Command.ArgValidator — tree-wide argument
// validation running after parsing and before the action. Test-only
// sub-package: references the new API, so it does not compile at base.
package vbvalidator

import (
	"context"
	"errors"
	"testing"

	cli "github.com/urfave/cli/v3"
)

func TestVBValidatorRunsBeforeAction(t *testing.T) {
	var order []string
	cmd := &cli.Command{
		Name: "app",
		ArgValidator: func(_ context.Context, _ *cli.Command) error {
			order = append(order, "validate")
			return nil
		},
		Action: func(_ context.Context, _ *cli.Command) error {
			order = append(order, "action")
			return nil
		},
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err != nil {
		t.Fatalf("run failed: %v", err)
	}
	if len(order) != 2 || order[0] != "validate" || order[1] != "action" {
		t.Fatalf("validator must run before the action, got %v", order)
	}
}

func TestVBValidatorErrorBlocksAction(t *testing.T) {
	sentinel := errors.New("rejected")
	actionRan := false
	cmd := &cli.Command{
		Name: "app",
		ArgValidator: func(_ context.Context, _ *cli.Command) error {
			return sentinel
		},
		Action: func(_ context.Context, _ *cli.Command) error {
			actionRan = true
			return nil
		},
	}
	err := cmd.Run(context.Background(), []string{"app", "whatever"})
	if err == nil {
		t.Fatal("validator error must fail the run")
	}
	if actionRan {
		t.Fatal("action must not run when validation fails")
	}
}

func TestVBValidatorInheritedBySubcommands(t *testing.T) {
	var seen []string
	root := &cli.Command{
		Name: "root",
		ArgValidator: func(_ context.Context, c *cli.Command) error {
			seen = append(seen, "root-validator:"+c.Name)
			return nil
		},
		Commands: []*cli.Command{
			{
				Name:   "sub",
				Action: func(_ context.Context, _ *cli.Command) error { return nil },
			},
		},
	}
	if err := root.Run(context.Background(), []string{"root", "sub"}); err != nil {
		t.Fatalf("run failed: %v", err)
	}
	if len(seen) == 0 {
		t.Fatal("parent's validator must apply to subcommands (tree-wide)")
	}
}

func TestVBChildValidatorOverridesParent(t *testing.T) {
	var called []string
	root := &cli.Command{
		Name: "root",
		ArgValidator: func(_ context.Context, _ *cli.Command) error {
			called = append(called, "parent")
			return nil
		},
		Commands: []*cli.Command{
			{
				Name: "sub",
				ArgValidator: func(_ context.Context, _ *cli.Command) error {
					called = append(called, "child")
					return nil
				},
				Action: func(_ context.Context, _ *cli.Command) error { return nil },
			},
		},
	}
	if err := root.Run(context.Background(), []string{"root", "sub"}); err != nil {
		t.Fatalf("run failed: %v", err)
	}
	if len(called) != 1 || called[0] != "child" {
		t.Fatalf("nearest validator wins; got %v", called)
	}
}

func TestVBValidatorSeesParsedArgs(t *testing.T) {
	cmd := &cli.Command{
		Name: "app",
		ArgValidator: func(_ context.Context, c *cli.Command) error {
			if c.Args().Len() != 2 {
				return errors.New("need exactly two args")
			}
			return nil
		},
		Action: func(_ context.Context, _ *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app", "a", "b"}); err != nil {
		t.Fatalf("two args must pass: %v", err)
	}
	if err := cmd.Run(context.Background(), []string{"app", "a"}); err == nil {
		t.Fatal("one arg must fail validation")
	}
}
