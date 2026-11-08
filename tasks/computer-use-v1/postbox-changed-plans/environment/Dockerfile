# Postbox task environment: the computer-use sandbox plus the in-house webmail
# client (a copy of tasks/computer-use-v1/_apps/postbox, kept identical by
# scripts/computer-use/sync_apps.py), this task's seed, seeded at build time
# so the pristine mailbox is the base artifact, and its arrivals schedule.
FROM vulcanbench/sandbox:cu-base

# Two messages arrive while the task runs, clocked from the agent's page
# views (app.py, POSTBOX_ARRIVALS).
ENV POSTBOX_ARRIVALS=/opt/postbox/arrivals.json

COPY postbox/ /opt/postbox/
RUN chmod 0755 /opt/postbox/entrypoint.sh /opt/postbox/collect.sh \
    && mkdir -p /var/lib/postbox/export /var/log/postbox \
    && python3 /opt/postbox/app.py --init /var/lib/postbox/app.db --seed /opt/postbox/seed.json

ENTRYPOINT ["/opt/postbox/entrypoint.sh"]
CMD ["sleep", "infinity"]
