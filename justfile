# Orchestrateur local minimal — `just` liste les recettes.

default:
    @just --list

# Nettoie les artefacts de build locaux (chemins déclarés — tools/clean.sh)
clean:
    sh tools/clean.sh
