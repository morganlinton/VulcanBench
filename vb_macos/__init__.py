"""Host-side tooling for the ComputerUse macOS suite (tasks/computer-use-macos-v1).

Each trial runs in a Tart macOS VM on the host (`vm`), driven through the VM's
own VNC server (`sandbox/computer-use/rfb.py`) and graded from the stopped
VM's disk mounted read-only. `env` is the harbor environment that ties a VM
to harbor's agent container. Standard library only, so harbor's own Python
can import it from the repo root (PYTHONPATH).
"""
