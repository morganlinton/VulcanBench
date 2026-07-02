When a registered teardown function raises an exception, the remaining teardown functions are silently skipped.

Flask lets you register multiple `teardown_request` and `teardown_appcontext` callbacks, and code can also subscribe to the `request_tearing_down` / `appcontext_tearing_down` signals. Teardown is documented as the place to release resources (close connections, remove sessions), so every registered callback must run on every request, no matter what.

Today, if one teardown callback raises, the callbacks and signal receivers that would have run after it never execute, so their resources leak. The raising callback's exception propagates alone and hides the fact that teardown was cut short.

Expected behavior: teardown must be robust. Every registered teardown callback and every teardown signal receiver runs even when earlier ones raise. Errors raised during teardown must not be swallowed; when one or more teardown callbacks raise, the collected errors should propagate together as an exception group (request teardown errors and app-context teardown errors each grouped appropriately). Teardown that raises nothing must behave exactly as it does today.
