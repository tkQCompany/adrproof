"""Runs only inside the neutral qualification sandbox; no host mutations."""
import ctypes
import errno
import json
import os
import socket
import sys
from pathlib import Path


def blocked_write(path):
    try:
        Path(path).write_text("synthetic probe")
    except OSError as error:
        return error.errno in (errno.EROFS, errno.EACCES, errno.EPERM)
    return False


host_marker, host_netns, port = sys.argv[1:]
checks = {
    "project_read_only": blocked_write("/project/forbidden.txt"),
    "spec_read_only": blocked_write("/spec/forbidden.txt"),
    "runtime_read_only": blocked_write("/tools/forbidden.txt"),
    "host_marker_hidden": not Path(host_marker).exists(),
    "host_home_hidden": not Path("/home").exists(),
    "host_environment_hidden": "ADRPROOF_PROBE_SECRET" not in os.environ,
    "private_network_namespace": os.readlink("/proc/self/ns/net") != host_netns,
    "fresh_state": list(Path("/state").iterdir()) == [],
}
with socket.socket() as connection:
    connection.settimeout(0.3)
    try:
        connection.connect(("127.0.0.1", int(port)))
        checks["host_service_unreachable"] = False
    except OSError:
        checks["host_service_unreachable"] = True
status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines())
checks["no_capabilities"] = int(status["CapEff"].strip(), 16) == 0
checks["no_new_privileges"] = status["NoNewPrivs"].strip() == "1"
libc = ctypes.CDLL(None, use_errno=True)
# Bubblewrap disables nesting via a namespace-count limit; kernels may report
# ENOSPC/EUSERS rather than a permissions error when that limit is reached.
checks["nested_userns_denied"] = libc.unshare(0x10000000) == -1 and ctypes.get_errno() in (errno.EPERM, errno.ENOSPC, errno.EUSERS)
Path("/state/allowed.txt").write_text("scratch")
Path("/tmp/allowed.txt").write_text("scratch")
checks["scratch_writable"] = True
print(json.dumps(checks, sort_keys=True))
sys.exit(0 if all(checks.values()) else 1)
