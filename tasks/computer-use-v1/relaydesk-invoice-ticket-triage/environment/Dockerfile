# Relay Desk task environment: the computer-use sandbox plus the in-house app,
# seeded at build time so the pristine database is the base artifact. Harbor
# runs the main service with `sleep infinity`; the ENTRYPOINT starts the app
# first (see app/entrypoint.sh).
FROM vulcanbench/sandbox:cu-base

COPY app/ /opt/relaydesk/
RUN chmod 0755 /opt/relaydesk/entrypoint.sh /opt/relaydesk/collect.sh \
    && mkdir -p /var/lib/relaydesk/export /var/log/relaydesk \
    && python3 /opt/relaydesk/app.py --init /var/lib/relaydesk/app.db --seed /opt/relaydesk/seed.json

ENTRYPOINT ["/opt/relaydesk/entrypoint.sh"]
CMD ["sleep", "infinity"]
