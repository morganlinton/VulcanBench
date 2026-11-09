# Invoice entry task environment: the computer-use sandbox (Thunar, Evince
# and Calc come from the base image) plus the seeded invoice folders and the
# payables ledger under /root. The PDFs, the ledger and the answer key are
# generated on the host by scripts/computer-use/seeds/files_invoice_entry.py
# and committed under invoices/. No browser is started for this task;
# harbor's computer-1 agent opens one anyway, and the entrypoint keeps the
# file manager on top of it.
FROM vulcanbench/sandbox:cu-base

ENV VB_NO_BROWSER=1 \
    VB_RAISE_WINDOW="incoming|Thunar"

COPY invoices/ /opt/invoices/
RUN chmod 0755 /opt/invoices/entrypoint.sh /opt/invoices/collect.sh \
    && mkdir -p /root/Invoices/incoming /root/Invoices/entered /root/Finance /var/lib/vbdesk/export \
    && cp /opt/invoices/pdfs/incoming/*.pdf /root/Invoices/incoming/ \
    && cp /opt/invoices/pdfs/entered/*.pdf /root/Invoices/entered/ \
    && cp /opt/invoices/Payables.ods /root/Finance/Payables.ods \
    && touch -d '2026-01-13 10:00:00' /root/Invoices/entered/*.pdf \
    && touch -d '2026-02-27 17:00:00' /root/Invoices/incoming/*.pdf \
    && touch -d '2026-02-27 17:30:00' /root/Finance/Payables.ods /root/Finance /root/Invoices /root/Invoices/incoming /root/Invoices/entered

ENTRYPOINT ["/opt/invoices/entrypoint.sh"]
CMD ["sleep", "infinity"]
