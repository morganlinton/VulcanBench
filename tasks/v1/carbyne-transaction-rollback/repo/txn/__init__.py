"""A tiny transactional DB wrapper. begin/commit/rollback bracket a unit of work."""


class DB:
    def __init__(self):
        self.log = []

    def begin(self):
        self.log.append("begin")

    def commit(self):
        self.log.append("commit")

    def rollback(self):
        self.log.append("rollback")
