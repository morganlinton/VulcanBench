# Cross-app task environment: the computer-use sandbox plus Relay Desk (a copy
# of tasks/computer-use-v1/_apps/relaydesk, kept identical by
# scripts/computer-use/sync_apps.py) with this task's seed, the ledger
# workbook and the empty report under /root/Finance (generated on the host by
# scripts/computer-use/seeds/ledger_reconcile.py). The entrypoints chain:
# Relay Desk's starts its app and execs the ledger one, which brings up the
# display, opens Calc and the editor, and execs the command (sleep infinity).
FROM vulcanbench/sandbox:cu-base

COPY relaydesk/ /opt/relaydesk/
COPY ledger/ /opt/ledger/
RUN chmod 0755 /opt/relaydesk/entrypoint.sh /opt/relaydesk/collect.sh /opt/ledger/entrypoint.sh /opt/ledger/collect.sh \
    && mkdir -p /var/lib/relaydesk/export /var/log/relaydesk /root/Finance /var/lib/vbdesk/export \
    && python3 /opt/relaydesk/app.py --init /var/lib/relaydesk/app.db --seed /opt/relaydesk/seed.json \
    && cp /opt/ledger/Q1-ledger.ods /root/Finance/Q1-ledger.ods \
    && cp /opt/ledger/discrepancies.txt /root/Finance/discrepancies.txt \
    && touch -d '2026-03-01 08:00:00' /root/Finance/Q1-ledger.ods /root/Finance/discrepancies.txt /root/Finance

ENTRYPOINT ["/opt/relaydesk/entrypoint.sh", "/opt/ledger/entrypoint.sh"]
CMD ["sleep", "infinity"]
