"""Local run GUI: discovery, first-attempt checks and (later) a launch queue.

Everything here wraps the existing CLI and scripts as subprocesses or reads
what they write to disk. Nothing in this package reads credentials; the
operator's environment is only ever passed through to a child process.
"""
