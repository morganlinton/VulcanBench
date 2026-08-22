// Hidden pass-to-pass guards: existing argument and flag behavior must not
// regress. Compiles and passes at the base commit.
package vbreg

import (
	"context"
	"testing"

	cli "github.com/urfave/cli/v3"
)

func TestVBOptionalSingleArgUsesDefault(t *testing.T) {
	var got string
	cmd := &cli.Command{
		Name: "app",
		Arguments: []cli.Argument{
			&cli.StringArg{Name: "id", Value: "fallback", Destination: &got},
		},
		Action: func(ctx context.Context, c *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err != nil {
		t.Fatalf("optional argument without a value must succeed, got %v", err)
	}
	if got != "fallback" {
		t.Fatalf("destination = %q, want fallback", got)
	}
}

func TestVBPluralArgsMinStillEnforced(t *testing.T) {
	var vals []string
	cmd := &cli.Command{
		Name: "app",
		Arguments: []cli.Argument{
			&cli.StringArgs{Name: "ids", Min: 1, Max: -1, Destination: &vals},
		},
		Action: func(ctx context.Context, c *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err == nil {
		t.Fatal("plural args below Min must return an error")
	}
	if err := cmd.Run(context.Background(), []string{"app", "a", "b"}); err != nil {
		t.Fatalf("plural args at or above Min must succeed, got %v", err)
	}
}

func TestVBRequiredFlagStillEnforced(t *testing.T) {
	cmd := &cli.Command{
		Name: "app",
		Flags: []cli.Flag{
			&cli.StringFlag{Name: "name", Required: true},
		},
		Action: func(ctx context.Context, c *cli.Command) error { return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err == nil {
		t.Fatal("missing required flag must return an error")
	}
	if err := cmd.Run(context.Background(), []string{"app", "--name", "x"}); err != nil {
		t.Fatalf("required flag given must succeed, got %v", err)
	}
}
