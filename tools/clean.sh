#!/bin/sh
# Nettoyage des artefacts de build — v2, adaptée au CARGO_TARGET_DIR partagé
# (~/.cargo/config.toml, 2026-08-08) : ./target n'existe plus en local, le
# nettoyage utile porte sur le répertoire PARTAGÉ, commun à tous les projets.
#   sh tools/clean.sh          # reliquats locaux + artefacts partagés
#                              # inutilisés depuis 7 jours (cargo sweep)
#   sh tools/clean.sh --all    # en plus : cargo clean = purge COMPLÈTE du
#                              # répertoire partagé (tous les projets du
#                              # poste recompileront)
# Chemins et commandes déclarés uniquement, jamais de balayage aveugle.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
ALL=0
case "${1:-}" in
    --all) ALL=1 ;;
    "") ;;
    *) echo "Usage : sh tools/clean.sh [--all]" >&2 ; exit 2 ;;
esac
total_kio=0
nettoyer() {
    if [ -e "$1" ]; then
        _kio="$(du -sk "$1" | cut -f1)"
        rm -rf "$1"
        total_kio=$((total_kio + _kio))
        echo "supprimé : $1 ($((_kio / 1024)) Mio)"
    fi
}
# Reliquats locaux (historiques, ou opt-out du partage) :
nettoyer target
echo "reliquats locaux : $((total_kio / 1024)) Mio libérés."
# Répertoire partagé : artefacts inutilisés depuis 7 jours.
PARTAGE="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | tr ',' '\n' | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)"
if [ -n "${PARTAGE}" ] && [ -d "${PARTAGE}" ]; then
    echo "répertoire partagé : ${PARTAGE} ($(du -sh "${PARTAGE}" | cut -f1))"
    if command -v cargo-sweep >/dev/null 2>&1; then
        cargo sweep --time 7
    else
        echo "note : cargo-sweep absent (cargo install cargo-sweep) — balayage ignoré." >&2
    fi
    if [ "${ALL}" -eq 1 ]; then
        echo "purge COMPLÈTE du répertoire partagé (--all) — tous les projets recompileront."
        cargo clean
    fi
    echo "état final : $(du -sh "${PARTAGE}" 2>/dev/null | cut -f1 || echo 0) dans ${PARTAGE}"
fi
