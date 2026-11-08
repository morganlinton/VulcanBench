# Relay Desk task environment: the computer-use sandbox plus the in-house app
# (a copy of tasks/computer-use-v1/_apps/relaydesk, kept identical by
# scripts/computer-use/sync_apps.py) and this task's seed, seeded at build time
# so the pristine database is the base artifact. Harbor runs the main service
# with `sleep infinity`; the ENTRYPOINT starts the app first (entrypoint.sh).
FROM vulcanbench/sandbox:cu-base

COPY relaydesk/ /opt/relaydesk/
RUN chmod 0755 /opt/relaydesk/entrypoint.sh /opt/relaydesk/collect.sh \
    && mkdir -p /var/lib/relaydesk/export /var/log/relaydesk \
    && python3 /opt/relaydesk/app.py --init /var/lib/relaydesk/app.db --seed /opt/relaydesk/seed.json

ENTRYPOINT ["/opt/relaydesk/entrypoint.sh"]
CMD ["sleep", "infinity"]
