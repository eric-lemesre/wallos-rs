# ADR 0019 — Authentification par jeton d'appareil (Bearer opaque, révocable)

## Contexte

REQ-AUT-005 exige que les coquilles natives (desktop/mobile) s'authentifient par un **jeton propre
à l'appareil**, révocable individuellement, plutôt que par le cookie de session web. AGENTS.md §9
fige : « jeton par appareil, révocable individuellement, stocké via `PlatformAdapter.secureStore` ;
JWT interdit (pas de révocation immédiate) ». L'infrastructure native (coquille Tauri,
`PlatformAdapter`, client SDK) est encore absente ; seule la partie serveur/API est livrée dans cet
incrément (cf. OQ-008).

## Décision

Introduire un mécanisme de **jeton d'appareil** homogène avec les sessions web :

- **Jeton opaque** (128 bits CSPRNG, comme les sessions), **jamais** un JWT. Seule son empreinte
  **SHA-256** est stockée (ADR 0018) ; le jeton clair n'est renvoyé qu'une fois, à l'appairage.
- Table `device_tokens` : `id` (révocation individuelle), `token_hash` unique, `user_id`,
  `household_id`, `label`, `platform`, `created_at`, `last_seen_at`. **Pas d'`expires_at`** : un
  appareil reste appairé jusqu'à révocation explicite (contrairement à l'inactivité glissante web).
- Endpoint public `POST /device-sessions` (`createDeviceSession`) : mêmes garde-fous que
  `POST /sessions` — limitation de taux (REQ-AUT-008) et vérification argon2 timing-safe (le cœur de
  login est factorisé dans `authenticate`) — mais émet le jeton dans le **corps JSON** (`DeviceToken`)
  au lieu d'un cookie.
- L'extracteur `AuthActor` accepte désormais **deux sources** : le cookie de session web **ou** un
  en-tête `Authorization: Bearer <token>` validé contre `device_tokens`. Le point de décision
  d'isolation reste l'`Actor` (ADR 0006/0012) : les deux chemins produisent le même contexte.
- La validation d'un jeton d'appareil rafraîchit `last_seen_at` (instant injecté), base de la
  colonne « dernière activité » de REQ-AUT-006.

## Conséquences

- La révocation est **immédiate** : supprimer la ligne `device_tokens` invalide le jeton au prochain
  appel (REQ-AUT-006), ce qu'un JWT ne permettrait pas — d'où son interdiction.
- REQ-AUT-007 pourra invalider en masse les jetons d'appareil (sauf la session courante) en opérant
  sur cette table.
- Le volet **client** (stockage via `PlatformAdapter.secureStore`, coquille native) est **différé**
  (OQ-008) : cet ADR ne couvre que le contrat serveur. REQ-AUT-005 reste donc `implemented`, pas
  `verified`, tant que le stockage natif n'est pas livré.
- Aucune dépendance nouvelle (réutilise `sha2`, `argon2`, `sqlx`, `uuid`).

## Liens

- AGENTS.md §9 (jeton par appareil, JWT interdit), §0 R6 ; ADR 0006 (Actor), 0012 (foyer), 0018
  (empreinte SHA-256). Exigences : REQ-AUT-005 (émission), REQ-AUT-006 (liste/révocation),
  REQ-AUT-007 (invalidation au changement de mot de passe). OQ-008 (volet natif différé).

## Statut

accepted
