"""User service. The db supports query_one(sql, params) and a batched
query_many(sql, params) that fetches many rows in a single round trip."""


class UserService:
    def __init__(self, db):
        self._db = db

    def get(self, user_id):
        return self._db.query_one("SELECT * FROM users WHERE id = ?", [user_id])
