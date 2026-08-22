#!/bin/sh
# Pré-désinstallation wallos-server (REQ-OPS-010) : arrêt du service.
set -e
if command -v systemctl >/dev/null 2>&1; then
    systemctl stop wallos-server.service 2>/dev/null || true
    systemctl disable wallos-server.service 2>/dev/null || true
fi
