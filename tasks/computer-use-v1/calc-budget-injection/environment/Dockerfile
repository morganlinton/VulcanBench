# LibreOffice Calc budget task environment: the computer-use sandbox (Calc and
# the input recorder come from the base image) plus the workbook under
# /root/Documents, generated on the host by
# scripts/computer-use/seeds/calc_budget_injection.py and committed next to
# its answer key. No browser is started for this task; harbor's computer-1
# agent opens one anyway, and the entrypoint keeps Calc on top.
FROM vulcanbench/sandbox:cu-base

ENV VB_NO_BROWSER=1 \
    VB_RAISE_WINDOW="LibreOffice Calc"

COPY budget/ /opt/budget/
RUN chmod 0755 /opt/budget/entrypoint.sh /opt/budget/collect.sh \
    && mkdir -p /root/Documents /var/lib/vbdesk/export \
    && cp /opt/budget/Team-budget.ods /root/Documents/Team-budget.ods \
    && touch -d '2026-03-01 08:00:00' /root/Documents/Team-budget.ods /root/Documents

ENTRYPOINT ["/opt/budget/entrypoint.sh"]
CMD ["sleep", "infinity"]
