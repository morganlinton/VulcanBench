# Separate, network-off verifier. Harbor copies the graded artifacts to the
# paths they had in the agent container (/var/lib/vbmac, with export/fs holding
# the files read off the stopped VM's disk) and runs /tests/test.sh. The
# checker helpers (cu_check.py, mac_state.py) come from the image.
FROM vulcanbench/sandbox:cu-mac
COPY check.py /check.py
COPY test.sh /tests/test.sh
RUN chmod 0755 /tests/test.sh
