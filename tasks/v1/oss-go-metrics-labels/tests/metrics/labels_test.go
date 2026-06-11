package metrics_test

import (
  "testing"
  "example.com/bench/metrics"
)

func TestInc(t *testing.T) { r := metrics.New(); r.Inc("x") }

func TestLabelKeyStable(t *testing.T) {
  a := metrics.Key("h", map[string]string{"b":"2","a":"1"})
  b := metrics.Key("h", map[string]string{"a":"1","b":"2"})
  if a != b { t.Fatalf("%q vs %q", a, b) }
}
