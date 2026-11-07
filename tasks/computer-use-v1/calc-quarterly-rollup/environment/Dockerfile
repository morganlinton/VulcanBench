# LibreOffice Calc rollup task environment: the computer-use sandbox (Calc and
# the input recorder come from the base image) plus the generated workbook
# under /root/Documents. No browser is started for this task; harbor's
# computer-1 agent opens one anyway, and the entrypoint keeps Calc on top.
FROM vulcanbench/sandbox:cu-base

ENV VB_NO_BROWSER=1 \
    VB_RAISE_WINDOW="LibreOffice Calc"

COPY rollup/ /opt/rollup/
RUN chmod 0755 /opt/rollup/entrypoint.sh /opt/rollup/collect.sh \
    && python3 /opt/rollup/make_workbook.py /opt/rollup/workbook.json /root/Documents/Q3-rollup.ods \
    && touch -d '2026-03-01 08:00:00' /root/Documents/Q3-rollup.ods /root/Documents \
    && mkdir -p /var/lib/vbdesk/export

ENTRYPOINT ["/opt/rollup/entrypoint.sh"]
CMD ["sleep", "infinity"]
