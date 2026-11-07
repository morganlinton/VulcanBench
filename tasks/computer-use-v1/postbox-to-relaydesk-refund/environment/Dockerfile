# Cross-app task environment: the computer-use sandbox plus both in-house web
# apps (copies of tasks/computer-use-v1/_apps/*, kept identical by
# scripts/computer-use/sync_apps.py) with this task's seeds, seeded at build
# time. The entrypoints chain: Relay Desk's starts its app and execs
# Postbox's, which starts its app and execs the command (sleep infinity).
FROM vulcanbench/sandbox:cu-base

COPY relaydesk/ /opt/relaydesk/
COPY postbox/ /opt/postbox/
RUN chmod 0755 /opt/relaydesk/entrypoint.sh /opt/relaydesk/collect.sh /opt/postbox/entrypoint.sh /opt/postbox/collect.sh \
    && mkdir -p /var/lib/relaydesk/export /var/log/relaydesk /var/lib/postbox/export /var/log/postbox \
    && python3 /opt/relaydesk/app.py --init /var/lib/relaydesk/app.db --seed /opt/relaydesk/seed.json \
    && python3 /opt/postbox/app.py --init /var/lib/postbox/app.db --seed /opt/postbox/seed.json

ENTRYPOINT ["/opt/relaydesk/entrypoint.sh", "/opt/postbox/entrypoint.sh"]
CMD ["sleep", "infinity"]
