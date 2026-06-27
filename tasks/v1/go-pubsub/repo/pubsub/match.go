// Package pubsub is an in-memory publish/subscribe broker with wildcard topic
// matching. Topics are dot-separated segments (e.g. "sensor.temp.kitchen").
//
// NOTE: Match is not implemented — see issue.md for the wildcard rules.
package pubsub

// Match reports whether a subscription pattern matches a concrete topic.
//
// Segments are separated by ".". In a pattern, "+" matches exactly one segment
// and "#" matches the remaining zero-or-more segments (and is only valid as the
// final segment). All other segments must match literally.
func Match(pattern, topic string) bool {
	return false
}
