// Hidden pass-to-pass guards: commands without a validator are unchanged.
package vbregv

import (
	"context"
	"errors"
	"testing"

	cli "github.com/urfave/cli/v3"
)

func TestVBPlainCommandStillRuns(t *testing.T) {
	ran := false
	cmd := &cli.Command{
		Name:   "app",
		Action: func(_ context.Context, _ *cli.Command) error { ran = true; return nil },
	}
	if err := cmd.Run(context.Background(), []string{"app", "x"}); err != nil || !ran {
		t.Fatalf("plain command broken: err=%v ran=%v", err, ran)
	}
}

func TestVBActionSeesArgs(t *testing.T) {
	var got []string
	cmd := &cli.Command{
		Name: "app",
		Action: func(_ context.Context, c *cli.Command) error {
			got = c.Args().Slice()
			return nil
		},
	}
	if err := cmd.Run(context.Background(), []string{"app", "a", "b"}); err != nil {
		t.Fatalf("run failed: %v", err)
	}
	if len(got) != 2 || got[0] != "a" || got[1] != "b" {
		t.Fatalf("args = %v", got)
	}
}

func TestVBActionErrorStillPropagates(t *testing.T) {
	sentinel := errors.New("boom")
	cmd := &cli.Command{
		Name:   "app",
		Action: func(_ context.Context, _ *cli.Command) error { return sentinel },
	}
	if err := cmd.Run(context.Background(), []string{"app"}); err == nil {
		t.Fatal("action error must propagate")
	}
}

func TestVBSubcommandDispatchWorks(t *testing.T) {
	hit := ""
	root := &cli.Command{
		Name: "root",
		Commands: []*cli.Command{
			{Name: "sub", Action: func(_ context.Context, _ *cli.Command) error { hit = "sub"; return nil }},
		},
	}
	if err := root.Run(context.Background(), []string{"root", "sub"}); err != nil || hit != "sub" {
		t.Fatalf("subcommand dispatch broken: err=%v hit=%q", err, hit)
	}
}
