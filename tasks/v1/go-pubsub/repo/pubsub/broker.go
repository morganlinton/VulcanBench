package pubsub

// NOTE: Subscribe and Publish are not implemented — see issue.md. The broker must
// be safe for concurrent use.

// Broker routes published messages to subscribers whose pattern matches the
// message's topic.
type Broker struct {
}

// New creates an empty broker.
func New() *Broker {
	return &Broker{}
}

// Subscribe registers fn to receive messages whose topic matches pattern, and
// returns a function that cancels the subscription.
func (b *Broker) Subscribe(pattern string, fn func(topic string, payload any)) func() {
	return func() {}
}

// Publish delivers payload to every subscription whose pattern matches topic, in
// subscription order.
func (b *Broker) Publish(topic string, payload any) {
}
