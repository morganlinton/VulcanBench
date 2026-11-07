# Separate, network-off verifier. Harbor copies the graded artifacts to the
# same paths they had in the environment (/root/Documents, /var/lib/vbdesk)
# and runs /tests/test.sh. The shared checker helpers come from the base
# image (/opt/vbdesk/cu_check.py).
FROM vulcanbench/sandbox:cu-base
COPY check.py seed-rollup.json /
COPY test.sh /tests/test.sh
RUN chmod 0755 /tests/test.sh
