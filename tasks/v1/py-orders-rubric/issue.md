# Add a method to list a user's orders

`ApiClient` (in `apiclient/client.py`) can fetch a single user and a single order.
Add a `list_orders(user_id, page=1)` method that returns the page of orders for a
user from `GET /users/{user_id}/orders?page={page}`.
