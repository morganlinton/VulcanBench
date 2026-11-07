# Separate, network-off verifier. Harbor copies the graded artifact to the same
# path it had in the environment (/var/lib/relaydesk) and runs /tests/test.sh.
FROM vulcanbench/sandbox:cu-base
COPY check.py seed.json /
COPY test.sh /tests/test.sh
RUN chmod 0755 /tests/test.sh
