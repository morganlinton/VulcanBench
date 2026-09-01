# Publish the expected flag value

`app.publish_flag()` round-trips a flag through the run's Redis service
(see `.vb_services.json` for the published port). The consumer expects the
flag value to be exactly `expected-42`; it currently is not.
