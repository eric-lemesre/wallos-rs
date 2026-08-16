# `@wallos/api-client`

Contrat TypeScript de l'API, **généré** depuis `api/openapi.json`.

```bash
npm run generate:api        # régénère src/schema.d.ts (depuis la racine du dépôt)
npm run ts-types-drift      # régénère ET échoue si le résultat diffère du committé (porte R8)
```

Aucun type d'entité métier ne s'écrit à la main : tout provient de `components["schemas"]`. Un
`interface Subscription` rédigé à la main est une erreur de CI, pas un choix de style — c'est la
protection principale contre la dérive silencieuse entre le serveur et l'interface.
