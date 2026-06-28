"""User repository. Every query goes through db.query(sql, params) with bound
parameters; SQL strings are never built by interpolation or concatenation."""


class UserRepo:
    def __init__(self, db):
        self.db = db

    def by_id(self, user_id):
        return self.db.query("SELECT * FROM users WHERE id = ?", [user_id])

    def by_status(self, status):
        return self.db.query("SELECT * FROM users WHERE status = ?", [status])
