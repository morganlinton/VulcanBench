// Package bucket provides a token-bucket rate limiter for throttling an API
// client. Tokens refill over time at a fixed rate, up to a fixed capacity.
//
// NOTE: this implementation has two bugs — refill() does not clamp the token
// count to capacity (so a long idle period grants a huge burst), and the bucket
// is not safe for concurrent use (Allow has no locking, so concurrent callers
// race on the token count and can over-grant). See issue.md.
package bucket

import "time"

// TokenBucket is a rate limiter. Construct it with New.
type TokenBucket struct {
	capacity float64
	rate     float64 // tokens added per second
	tokens   float64
	last     time.Time
	now      func() time.Time
}

// New returns a bucket that starts full, refills at ratePerSec tokens/second up
// to capacity, and reads the current time from now (injected for testing).
func New(capacity, ratePerSec float64, now func() time.Time) *TokenBucket {
	return &TokenBucket{
		capacity: capacity,
		rate:     ratePerSec,
		tokens:   capacity,
		last:     now(),
		now:      now,
	}
}

func (b *TokenBucket) refill() {
	t := b.now()
	elapsed := t.Sub(b.last).Seconds()
	b.tokens += elapsed * b.rate
	b.last = t
}

// Allow reports whether one token is available, consuming it if so.
func (b *TokenBucket) Allow() bool { return b.AllowN(1) }

// AllowN reports whether n tokens are available, consuming them if so.
func (b *TokenBucket) AllowN(n float64) bool {
	b.refill()
	if b.tokens >= n {
		b.tokens -= n
		return true
	}
	return false
}
