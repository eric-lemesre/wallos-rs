#!/bin/sh
# Post-désinstallation wallos-server (REQ-OPS-010).
# Désinstallation ordinaire : données (/var/lib/wallos-server) et configuration
# (/etc/wallos-server) sont CONSERVÉES. Leur suppression exige la purge
# explicite (`apt purge` → argument "purge" ; sous rpm, pas de purge : la
# conservation est la règle).
set -e
case "${1:-}" in
    purge)
        rm -rf /etc/wallos-server /var/lib/wallos-server
        if getent passwd wallos-server >/dev/null; then
            userdel wallos-server || true
        fi
        ;;
    *)
        : # conservation des données et de la configuration
        ;;
esac
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi
