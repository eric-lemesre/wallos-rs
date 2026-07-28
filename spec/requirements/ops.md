---
id: REQ-OPS-001
title: Endpoint de santé
domain: ops
status: verified
criticality: low
layer:
- api
e2e: optional
oracle: design
rationale: >
  Fournir un point de contrôle simple permettant de vérifier que le serveur est démarré
  et répond aux requêtes HTTP.
acceptance:
  - given: le serveur est démarré
    when: on requête GET /health
    then: la réponse est 200 avec le corps "ok"
depends_on: []
---
