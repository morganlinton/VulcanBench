# Thunar filing task environment: the computer-use sandbox (Thunar and the
# input recorder come from the base image) plus the seeded file tree under
# /root. No browser is started for this task; harbor's computer-1 agent opens
# one anyway, and the entrypoint keeps Thunar on top of it.
FROM vulcanbench/sandbox:cu-base

ENV VB_NO_BROWSER=1 \
    VB_RAISE_WINDOW="Downloads|Thunar"

COPY filing/ /opt/filing/
RUN chmod 0755 /opt/filing/entrypoint.sh /opt/filing/collect.sh \
    && python3 /opt/filing/seed_fs.py /opt/filing/seed.json /root \
    && mkdir -p /var/lib/vbdesk/export

ENTRYPOINT ["/opt/filing/entrypoint.sh"]
CMD ["sleep", "infinity"]
