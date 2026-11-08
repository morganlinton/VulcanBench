# Postbox task environment: the computer-use sandbox plus the in-house webmail
# client (a copy of tasks/computer-use-v1/_apps/postbox, kept identical by
# scripts/computer-use/sync_apps.py) and this task's seed, seeded at build time
# so the pristine mailbox is the base artifact. Harbor runs the main service
# with `sleep infinity`; the ENTRYPOINT starts the app first (entrypoint.sh).
FROM vulcanbench/sandbox:cu-base

COPY postbox/ /opt/postbox/
RUN chmod 0755 /opt/postbox/entrypoint.sh /opt/postbox/collect.sh \
    && mkdir -p /var/lib/postbox/export /var/log/postbox \
    && python3 /opt/postbox/app.py --init /var/lib/postbox/app.db --seed /opt/postbox/seed.json

ENTRYPOINT ["/opt/postbox/entrypoint.sh"]
CMD ["sleep", "infinity"]
