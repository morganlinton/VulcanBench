"""ofkit: the in-house platform layer shared by every orderflow service.

Zero third-party runtime dependencies beyond psycopg2 and requests: HTTP
serving is stdlib, Redis speaks RESP over a socket, config comes from the
environment, and topology comes from the run manifests. Keep it boring.
"""
