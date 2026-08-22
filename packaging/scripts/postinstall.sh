#!/bin/sh
# Post-installation wallos-server (REQ-OPS-010) — deb et rpm (nfpm).
set -e

# Compte système dédié, non privilégié, sans session interactive.
if ! getent passwd wallos-server >/dev/null; then
    useradd --system --user-group --no-create-home \
        --home-dir /var/lib/wallos-server \
        --shell /usr/sbin/nologin wallos-server
fi

# La configuration (porteuse de secrets une fois renseignée) n'est lisible
# que du compte de service.
chgrp wallos-server /etc/wallos-server/wallos-server.env || true
chmod 0640 /etc/wallos-server/wallos-server.env || true

if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
    # Mise à jour : redémarre le service s'il est actif — jamais de démarrage
    # imposé à l'installation (aucun secret prédéfini, il échouerait de toute
    # façon tant que DATABASE_URL n'est pas renseignée).
    systemctl try-restart wallos-server.service || true
fi

echo "wallos-server : renseigner /etc/wallos-server/wallos-server.env puis :"
echo "  systemctl enable --now wallos-server"
