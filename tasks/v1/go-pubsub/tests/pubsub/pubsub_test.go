package pubsub

import (
	"sync"
	"testing"
)

func TestMatchLiteralAndWildcards(t *testing.T) {
	cases := []struct {
		pattern, topic string
		want           bool
	}{
		{"a.b.c", "a.b.c", true},
		{"a.b.c", "a.b.d", false},
		{"a.b", "a.b.c", false},     // too short
		{"a.b.c", "a.b", false},     // too long
		{"a.+.c", "a.b.c", true},    // + matches one segment
		{"a.+.c", "a.x.c", true},
		{"a.+.c", "a.c", false},     // + must match exactly one
		{"a.+.c", "a.b.x.c", false}, // + is one segment, not two
		{"a.#", "a.b.c", true},      // # matches the rest
		{"a.#", "a", true},          // # matches zero trailing segments
		{"a.#", "b.c", false},       // literal prefix must still match
		{"#", "a.b.c", true},        // # alone matches anything
		{"a.+", "a", false},         // + needs a segment
		{"a.+", "a.b", true},
	}
	for _, tc := range cases {
		if got := Match(tc.pattern, tc.topic); got != tc.want {
			t.Errorf("Match(%q, %q) = %v, want %v", tc.pattern, tc.topic, got, tc.want)
		}
	}
}

func TestPublishDeliversToMatchingSubscribers(t *testing.T) {
	b := New()
	var got []string
	b.Subscribe("sensor.+.kitchen", func(topic string, _ any) { got = append(got, "k:"+topic) })
	b.Subscribe("sensor.#", func(topic string, _ any) { got = append(got, "all:"+topic) })
	b.Subscribe("other.#", func(topic string, _ any) { got = append(got, "other:"+topic) })

	b.Publish("sensor.temp.kitchen", 21)
	// both kitchen and sensor.# match, in subscription order; other.# does not
	want := []string{"k:sensor.temp.kitchen", "all:sensor.temp.kitchen"}
	if len(got) != len(want) {
		t.Fatalf("got %v, want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("got %v, want %v", got, want)
		}
	}
}

func TestPublishPassesPayload(t *testing.T) {
	b := New()
	var seen any
	b.Subscribe("a.b", func(_ string, payload any) { seen = payload })
	b.Publish("a.b", 42)
	if seen != 42 {
		t.Fatalf("payload = %v, want 42", seen)
	}
}

func TestUnsubscribeStopsDelivery(t *testing.T) {
	b := New()
	n := 0
	cancel := b.Subscribe("a.#", func(_ string, _ any) { n++ })
	b.Publish("a.b", nil)
	cancel()
	b.Publish("a.b", nil)
	if n != 1 {
		t.Fatalf("delivery count = %d, want 1 (after unsubscribe)", n)
	}
}

func TestPublishNoSubscribersIsNoop(t *testing.T) {
	b := New()
	b.Publish("a.b.c", nil) // must not panic
}

func TestConcurrentSubscribePublishIsRaceFree(t *testing.T) {
	b := New()
	var wg sync.WaitGroup
	for i := 0; i < 50; i++ {
		wg.Add(2)
		go func() { defer wg.Done(); cancel := b.Subscribe("a.#", func(_ string, _ any) {}); cancel() }()
		go func() { defer wg.Done(); b.Publish("a.b.c", nil) }()
	}
	wg.Wait()
}
