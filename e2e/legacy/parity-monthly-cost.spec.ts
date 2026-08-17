import { expect, test } from "@playwright/test";

import { createHarness } from "../drivers/Harness";
import oracle from "../fixtures/oracles/REQ-STA-001-monthly.json" with { type: "json" };

/**
 * Étape 2 du protocole d'oracle (AGENTS.md §8.1) : **vérifier la référence avant de s'y fier**.
 *
 * Les valeurs de `REQ-STA-001-monthly.json` provenaient de l'exécution du PHP de l'image
 * (`_evidence: executed`) — solide sur la règle de calcul, muet sur ce que l'utilisateur voit. Ce
 * scénario les confronte au comportement **rendu** par l'application d'origine en marche, et promeut
 * l'oracle au niveau `observed` (ADR 0059, R11).
 *
 * Il ne teste pas subtrack : il teste ce contre quoi subtrack est testé. C'est le maillon qui
 * manquait — un oracle faux rendrait nos tests verts sans que rien le signale.
 *
 * **Pourquoi une somme courante plutôt qu'un test par vecteur.** L'endpoint de suppression de Wallos
 * exige un jeton anti-CSRF, et repartir d'un conteneur neuf à chaque vecteur coûterait une minute
 * par assertion. Or le coût mensuel total est additif : créer les abonnements l'un après l'autre et
 * vérifier le cumul à chaque étape se passe de tout nettoyage, et se trouve **plus exigeant** — cela
 * contrôle aussi la façon dont l'original agrège et arrondit, ce qu'un vecteur isolé ne montre pas.
 *
 * **Précondition : une instance vierge.** Le cumul part de zéro. En intégration continue, le
 * conteneur est toujours neuf ; en local, `docker compose -f legacy/docker-compose.yml down && up`
 * avant de rejouer. Partir du total constaté serait plus tolérant mais ajouterait des valeurs
 * brutes à une base déjà arrondie, ce qui fausserait l'arrondi final.
 */
const PRECISION = 10_000; // 1/10 000 d'unité : les valeurs brutes vont jusqu'à 4 décimales

/** Arrondi au centime, mi-unité au pair — règle capturée en REQ-CUR-005. */
function auCentimePair(dixMillièmes: number): string {
  const centimes = dixMillièmes / 100;
  const bas = Math.floor(centimes);
  const reste = centimes - bas;
  let arrondi: number;
  if (reste > 0.5) arrondi = bas + 1;
  else if (reste < 0.5) arrondi = bas;
  else arrondi = bas % 2 === 0 ? bas : bas + 1;
  return (arrondi / 100).toFixed(2);
}

test.describe("Oracle — coût mensuel normalisé", { tag: ["@legacy-oracle", "@REQ-STA-001"] }, () => {
  // Un seul navigateur, à dessein. La règle §8.2 impose chromium ET webkit pour NOTRE interface,
  // dont le rendu dépend du moteur. Ici le calcul est fait par le serveur PHP et le navigateur ne
  // fait que le lire : un second passage n'apprendrait rien. Il nuirait même — les deux projets
  // partagent l'unique instance Wallos, si bien que le second repartirait d'un état déjà peuplé et
  // invaliderait le cumul.
  test.skip(
    ({ browserName }) => browserName !== "chromium",
    "vérification d'un calcul serveur : la diversité de moteurs n'apporte rien et casse l'isolation",
  );

  test("l'application d'origine rend les valeurs gelées, et les cumule", async ({
    page,
    baseURL,
  }) => {
    const app = createHarness(page, baseURL!, "legacy");
    await app.signIn({ email: "e2e@example.com", password: "correcthorsebatterystaple" });

    let cumul = 0;
    for (const v of oracle.vectors) {
      await app.createSubscription({
        name: `Oracle ${v.unit}-${v.interval}-${v.price}`,
        amount: v.price,
        currency: "EUR",
        unit: v.unit as "day" | "week" | "month" | "year",
        interval: String(v.interval),
        // Échéance future fixe : la normalisation mensuelle n'en dépend pas, et une date relative à
        // « aujourd'hui » rendrait le scénario dépendant du jour de son exécution.
        firstPayment: "2027-01-15",
      });

      cumul += Math.round(Number(v.monthly) * PRECISION);
      expect(await app.readMonthlyTotal(), `après « ${v.why} »`).toBe(auCentimePair(cumul));
    }
  });
});
