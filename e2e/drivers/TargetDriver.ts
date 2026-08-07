import type { Page } from "@playwright/test";

import type { AppDriver, MoneyInput, SignupInput } from "./AppDriver";
import type { Credentials, Harness } from "./Harness";

/**
 * Pilote `subtrack` via les `data-testid` stables (les sélecteurs CSS/XPath sont interdits, §7).
 */
export class TargetDriver implements AppDriver, Harness {
  constructor(
    private readonly page: Page,
    private readonly baseURL: string,
  ) {}

  // --- Contrat agnostique partagé (Harness, §8.1) ---

  async signIn({ email, password }: Credentials): Promise<void> {
    await this.gotoSignup();
    await this.signup({ email, password });
    await this.login({ email, password });
  }

  async signedIn(): Promise<boolean> {
    return this.currentUserVisible();
  }

  async gotoSignup(): Promise<void> {
    await this.page.goto(this.baseURL);
  }

  async signup({ email, password }: SignupInput): Promise<void> {
    await this.page.getByTestId("signup-email").fill(email);
    await this.page.getByTestId("signup-password").fill(password);
    await this.page.getByTestId("signup-submit").click();
  }

  async signupSucceeded(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("signup-success")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async hasPasswordError(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("signup-password-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async login({ email, password }: SignupInput): Promise<void> {
    await this.page.getByTestId("login-email").fill(email);
    await this.page.getByTestId("login-password").fill(password);
    await this.page.getByTestId("login-submit").click();
  }

  async currentUserVisible(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("login-current-user")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async loginFailed(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("login-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async logout(): Promise<void> {
    await this.page.getByTestId("logout").click();
  }

  async currentUserGone(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("login-current-user")
        .waitFor({ state: "detached", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Gestion des appareils (REQ-AUT-006) ---

  async pairDevice(
    { email, password }: SignupInput,
    label: string,
    platform: string,
  ): Promise<void> {
    // Appairage via l'API depuis le contexte de la page (même origine + cookie de session),
    // comme le ferait une coquille native présentant email/mot de passe.
    await this.page.evaluate(
      async ([email, password, label, platform]) => {
        await fetch("/api/v1/device-sessions", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ email, password, label, platform }),
        });
      },
      [email, password, label, platform] as const,
    );
  }

  async openDevices(): Promise<void> {
    await this.page.reload();
  }

  private deviceRow(label: string) {
    return this.page.getByTestId("device-row").filter({ hasText: label });
  }

  async deviceListed(label: string): Promise<boolean> {
    try {
      await this.deviceRow(label).waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async revokeDevice(label: string): Promise<void> {
    await this.deviceRow(label).getByRole("button").click();
  }

  async deviceGone(label: string): Promise<boolean> {
    try {
      await this.deviceRow(label).waitFor({ state: "detached", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Changement de mot de passe (REQ-AUT-007) ---

  async changePassword(current: string, next: string): Promise<void> {
    await this.page.getByTestId("change-password-current").fill(current);
    await this.page.getByTestId("change-password-new").fill(next);
    await this.page.getByTestId("change-password-submit").click();
  }

  async passwordChangeSucceeded(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("change-password-success")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Création d'abonnement (REQ-SUB-002) ---

  async createSubscription(input: {
    name: string; amount: string; currency: string; unit: string; interval: string; firstPayment: string; endDate?: string; trialEnd?: string; category?: string;
  }): Promise<void> {
    await this.page.getByTestId("sub-name").fill(input.name);
    await this.page.getByTestId("sub-amount").fill(input.amount);
    await this.page.getByTestId("sub-currency").fill(input.currency);
    await this.page.getByTestId("sub-cycle-unit").selectOption(input.unit);
    await this.page.getByTestId("sub-cycle-interval").fill(input.interval);
    await this.page.getByTestId("sub-first-payment").fill(input.firstPayment);
    if (input.endDate !== undefined) {
      await this.page.getByTestId("sub-end-date").fill(input.endDate);
    }
    if (input.trialEnd !== undefined) {
      await this.page.getByTestId("sub-trial-end").fill(input.trialEnd);
    }
    // Rattachement à une catégorie (par nom) : le sélecteur est peuplé depuis GET /categories.
    if (input.category !== undefined) {
      await this.page.getByTestId("sub-category").selectOption({ label: input.category });
    }
    await this.page.getByTestId("sub-submit").click();
  }

  async subscriptionEnded(name: string): Promise<boolean> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    try {
      await row.getByTestId("subscription-ended").waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async subscriptionNextPayment(): Promise<string> {
    await this.page.getByTestId("sub-success").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("sub-success").textContent()) ?? "";
  }

  // --- Échéancier des prochains paiements (REQ-STA-005) ---

  async loadUpcoming(days: string): Promise<void> {
    await this.page.getByTestId("upcoming-days").fill(days);
    await this.page.getByTestId("upcoming-load").click();
  }

  /** Nombre d'occurrences de l'échéancier portant le nom `name`. */
  async upcomingOccurrences(name: string): Promise<number> {
    const list = this.page.getByTestId("upcoming-list");
    try {
      await list.waitFor({ state: "visible", timeout: 5000 });
    } catch {
      return 0;
    }
    return this.page
      .getByTestId("upcoming-row")
      .filter({ hasText: name })
      .count();
  }

  // --- Liste et filtres (REQ-SUB-006) ---

  async refreshSubscriptions(): Promise<void> {
    await this.page.getByTestId("subscriptions-apply").click();
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
  }

  async filterSubscriptionsByCategory(category: string): Promise<void> {
    await this.page.getByTestId("subscriptions-filter-category").fill(category);
    await this.page.getByTestId("subscriptions-apply").click();
  }

  async subscriptionListed(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("subscription-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async subscriptionsEmpty(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("subscriptions-empty")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async subscriptionsTotal(): Promise<string> {
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("subscriptions-total").textContent()) ?? "";
  }

  // --- Évolution du coût sur 12 mois (REQ-STA-006) ---

  async reloadCostEvolution(): Promise<void> {
    await this.page.getByTestId("evolution-reload").click();
  }

  /** Nombre de points affichés dans la série d'évolution (instantané ; encapsuler dans expect.poll). */
  async costEvolutionPointCount(): Promise<number> {
    return this.page.getByTestId("evolution-point").count();
  }

  /** Total affiché pour le mois `yyyyMm` (`YYYY-MM`), ou chaîne vide s'il est absent. */
  async costEvolutionTotal(yyyyMm: string): Promise<string> {
    const point = this.page
      .getByTestId("evolution-point")
      .filter({ has: this.page.getByTestId("evolution-month").filter({ hasText: yyyyMm }) });
    if ((await point.count()) === 0) {
      return "";
    }
    return (await point.getByTestId("evolution-total").innerText()) ?? "";
  }

  // --- Répartition par catégorie et par payeur (REQ-STA-004) ---

  async reloadRepartition(): Promise<void> {
    await this.page.getByTestId("repartition-reload").click();
  }

  /** Total général affiché par la carte de répartition, ou chaîne vide s'il est absent (instantané). */
  async repartitionGrandTotal(): Promise<string> {
    const total = this.page.getByTestId("repartition-grand-total");
    if ((await total.count()) === 0) {
      return "";
    }
    return (await total.innerText()) ?? "";
  }

  /** Libellés des entrées d'un axe (`category` | `payer`), dans l'ordre du DOM (instantané). */
  async repartitionLabels(axis: "category" | "payer"): Promise<string[]> {
    return this.page
      .getByTestId(`repartition-entry-${axis}`)
      .getByTestId("repartition-label")
      .allInnerTexts();
  }

  // --- Recherche et tri (REQ-SUB-007) ---

  async searchSubscriptions(term: string): Promise<void> {
    await this.page.getByTestId("subscriptions-search").fill(term);
    await this.page.getByTestId("subscriptions-apply").click();
  }

  async sortSubscriptionsBy(sort: "name" | "amount" | "next_due"): Promise<void> {
    await this.page.getByTestId("subscriptions-sort").selectOption(sort);
    await this.page.getByTestId("subscriptions-apply").click();
  }

  /**
   * Noms des abonnements affichés, **dans l'ordre du DOM** (pour vérifier le tri).
   *
   * Instantané non bloquant : la liste étant rechargée de façon asynchrone après « Appliquer », le
   * réordonnancement/filtrage n'est pas forcément visible au premier appel. L'appelant enveloppe cette
   * lecture dans un `expect.poll(...)` (assertion web-first à réessai) pour absorber la course de rendu.
   */
  async subscriptionNames(): Promise<string[]> {
    return this.page.getByTestId("subscription-name").allInnerTexts();
  }

  // --- Cohérence total / filtre (REQ-STA-007) ---

  async totalIndicatesFiltered(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("subscriptions-total-filtered")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async filterSubscriptionsByState(state: "all" | "active" | "inactive"): Promise<void> {
    await this.page.getByTestId("subscriptions-filter-state").selectOption(state);
    await this.page.getByTestId("subscriptions-apply").click();
  }

  // --- Modification (REQ-SUB-004) ---

  async editSubscriptionAmount(name: string, amount: string): Promise<void> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-edit").click();
    await row.getByTestId("subscription-amount-input").fill(amount);
    await row.getByTestId("subscription-save").click();
    // Attend que la liste rechargée reflète le nouveau montant (recalcul serveur appliqué).
    await this.page
      .getByTestId("subscription-amount")
      .filter({ hasText: amount })
      .first()
      .waitFor({ state: "visible", timeout: 5000 });
  }

  async subscriptionAmount(name: string): Promise<string> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-amount").waitFor({ state: "visible", timeout: 5000 });
    return (await row.getByTestId("subscription-amount").textContent()) ?? "";
  }

  // --- Suppression (REQ-SUB-005) ---

  /** Clique « Supprimer » sur la ligne de l'abonnement identifié par son nom. */
  async deleteSubscription(name: string): Promise<void> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId(/^subscription-delete-/).click();
  }

  // --- Coût mensuel normalisé (REQ-STA-001) ---

  async subscriptionMonthlyCost(name: string): Promise<string> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-monthly").waitFor({ state: "visible", timeout: 5000 });
    return (await row.getByTestId("subscription-monthly").textContent()) ?? "";
  }

  async subscriptionYearlyCost(name: string): Promise<string> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-yearly").waitFor({ state: "visible", timeout: 5000 });
    return (await row.getByTestId("subscription-yearly").textContent()) ?? "";
  }

  // --- Abonnement désactivé (REQ-SUB-008) ---

  async deactivateSubscription(name: string): Promise<void> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-edit").click();
    await row.getByTestId("subscription-active").uncheck();
    await row.getByTestId("subscription-save").click();
    // Attend que la liste rechargée reflète l'exclusion du total (total nul).
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
  }

  /** Réactive un abonnement désactivé (REQ-STA-003 critère #2 : réintégration immédiate). */
  async reactivateSubscription(name: string): Promise<void> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-edit").click();
    await row.getByTestId("subscription-active").check();
    await row.getByTestId("subscription-save").click();
    // Attend que la liste rechargée soit visible (le total reflète alors la réintégration).
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
  }

  // --- Calcul d'échéance (REQ-SUB-012) ---

  async computeNextDue(anchor: string, unit: string, interval: string, after: string): Promise<void> {
    await this.page.getByTestId("nextdue-anchor").fill(anchor);
    await this.page.getByTestId("nextdue-unit").selectOption(unit);
    await this.page.getByTestId("nextdue-interval").fill(interval);
    await this.page.getByTestId("nextdue-after").fill(after);
    await this.page.getByTestId("nextdue-compute").click();
  }

  async readNextDue(): Promise<string> {
    await this.page.getByTestId("nextdue-result").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("nextdue-result").textContent()) ?? "";
  }

  // --- Catégories (REQ-CAT-001) ---

  async createCategory(name: string): Promise<void> {
    await this.page.getByTestId("category-new-name").fill(name);
    await this.page.getByTestId("category-create").click();
  }

  async categoryVisible(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("category-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async categoryAbsent(name: string): Promise<boolean> {
    // La liste doit être chargée (section visible) avant de conclure à l'absence.
    await this.page.getByTestId("categories-list").waitFor({ state: "visible", timeout: 5000 });
    return (
      (await this.page.getByTestId("category-name").filter({ hasText: name }).count()) === 0
    );
  }

  /** Clique « Supprimer » sur la ligne de la catégorie identifiée par son nom (REQ-CAT-003). */
  async deleteCategory(name: string): Promise<void> {
    const row = this.page.getByTestId("category-row").filter({ hasText: name });
    await row.getByTestId(/^category-delete-/).click();
  }

  /** Vrai si le refus de suppression (catégorie référencée, 409) est affiché (REQ-CAT-003). */
  async categoryDeleteRefused(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("category-delete-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Payeurs (REQ-SUB-017) ---

  async createPayer(name: string): Promise<void> {
    await this.page.getByTestId("payer-new-name").fill(name);
    await this.page.getByTestId("payer-create").click();
  }

  async payerVisible(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("payer-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  /** Rattache un abonnement à un payeur (par nom) via l'API, comme le ferait le formulaire. */
  async attachSubscriptionToPayer(subName: string, payerName: string): Promise<void> {
    await this.page.evaluate(
      async ([subName, payerName]) => {
        const payers = (await (await fetch("/api/v1/payers")).json()) as {
          id: string;
          name: string;
        }[];
        const payer = payers.find((p) => p.name === payerName);
        await fetch("/api/v1/subscriptions", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            name: subName,
            amount: "9.99",
            currency: "EUR",
            cycle: { unit: "month", interval: 1 },
            first_payment: "2030-01-15",
            payer: payer?.id,
          }),
        });
      },
      [subName, payerName] as const,
    );
  }

  /** Clique « Supprimer » sur la ligne du payeur identifié par son nom (REQ-SUB-017). */
  async deletePayer(name: string): Promise<void> {
    const row = this.page.getByTestId("payer-row").filter({ hasText: name });
    await row.getByTestId(/^payer-delete-/).click();
  }

  /** Vrai si le refus de suppression (payeur référencé, 409) est affiché (REQ-SUB-017). */
  async payerDeleteRefused(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("payer-delete-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Synchronisation : pierres tombales (REQ-SYN-002) ---

  /**
   * Récupère les pierres tombales via l'endpoint de synchronisation (API-only, sans UI dédiée), comme
   * le ferait un client offline-first au réveil. Renvoie les types d'entités supprimées.
   */
  async tombstonedEntityTypes(): Promise<string[]> {
    return this.page.evaluate(async () => {
      const res = await fetch("/api/v1/sync/tombstones");
      const body = (await res.json()) as { tombstones: { entity_type: string }[] };
      return body.tombstones.map((t) => t.entity_type);
    });
  }

  // --- Récupération incrémentale par curseur (REQ-SYN-003) ---

  /**
   * Draine **toutes** les pages du delta incrémental depuis l'origine, avec une taille de page donnée,
   * en suivant `next_cursor` jusqu'à `has_more = false` (comme un client offline-first). Renvoie la
   * liste `entity_type:id` de tous les changements, pour vérifier la stabilité de la pagination.
   */
  async drainSyncChanges(pageSize: number): Promise<string[]> {
    return this.page.evaluate(async (page) => {
      const keys: string[] = [];
      let cursor: string | null = null;
      // Garde-fou : borne le nombre de pages pour ne jamais boucler indéfiniment en test.
      for (let i = 0; i < 100; i++) {
        const q = cursor
          ? `?limit=${page}&cursor=${encodeURIComponent(cursor)}`
          : `?limit=${page}`;
        const res = await fetch(`/api/v1/sync/changes${q}`);
        const body = (await res.json()) as {
          changes: { entity_type: string; id: string }[];
          next_cursor: string;
          has_more: boolean;
        };
        for (const c of body.changes) keys.push(`${c.entity_type}:${c.id}`);
        if (!body.has_more) break;
        cursor = body.next_cursor;
      }
      return keys;
    }, pageSize);
  }

  // --- Poussée des modifications locales (REQ-SYN-004) ---

  /**
   * Pousse un lot d'opérations locales via l'endpoint de synchronisation (comme un client offline-first
   * au retour du réseau), avec une clé d'idempotence optionnelle. Renvoie les statuts par opération.
   */
  async pushSyncChanges(
    operations: unknown[],
    idempotencyKey?: string,
  ): Promise<string[]> {
    return this.page.evaluate(
      async ({ operations, idempotencyKey }) => {
        const headers: Record<string, string> = { "content-type": "application/json" };
        if (idempotencyKey) headers["idempotency-key"] = idempotencyKey;
        const res = await fetch("/api/v1/sync/push", {
          method: "POST",
          headers,
          body: JSON.stringify({ operations }),
        });
        const body = (await res.json()) as { results: { status: string }[] };
        return body.results.map((r) => r.status);
      },
      { operations, idempotencyKey },
    );
  }

  // --- Rappels avant échéance (REQ-NOT-001) ---

  /** Noms des rappels dus aujourd'hui affichés dans la carte des rappels. */
  async reminderNames(): Promise<string[]> {
    return this.page
      .getByTestId("reminders-card")
      .getByTestId("reminder-name")
      .allInnerTexts();
  }

  // --- Ordonnanceur de rappels (REQ-NOT-002) ---

  /**
   * Déclenche l'ordonnanceur de rappels (endpoint d'opérateur, secret injecté par la config e2e)
   * et renvoie le nombre de rappels émis. `asOf` : date de référence YYYY-MM-DD.
   */
  async runReminders(asOf: string, cronToken: string): Promise<number> {
    return this.page.evaluate(
      async ({ asOf, cronToken }) => {
        const res = await fetch(`/api/v1/internal/run-reminders?as_of=${asOf}`, {
          method: "POST",
          headers: { "x-cron-token": cronToken },
        });
        const body = (await res.json()) as { emitted: number };
        return body.emitted;
      },
      { asOf, cronToken },
    );
  }

  // --- Canaux de notification (REQ-NOT-005/006) ---

  /** Ajoute un canal webhook depuis la carte des canaux de notification. */
  async addWebhookChannel(url: string): Promise<void> {
    await this.page.getByTestId("notification-channel-type").selectOption("webhook");
    await this.page.getByTestId("notification-channel-url").fill(url);
    await this.page.getByTestId("notification-channel-add").click();
  }

  /** Déclenche l'envoi de test du premier canal listé. */
  async testFirstChannel(): Promise<void> {
    await this.page
      .getByTestId("notification-channel-row")
      .first()
      .locator('button[data-testid^="notification-channel-test-"]')
      .click();
  }

  /** Résultat du dernier test de canal : `{ ok, message }`, ou `null` si non affiché. */
  async channelTestResult(): Promise<{ ok: boolean; message: string } | null> {
    const result = this.page.getByTestId("notification-channel-test-result");
    if ((await result.count()) === 0) {
      return null;
    }
    return {
      ok: (await result.getAttribute("data-ok")) === "true",
      message: (await result.innerText()).trim(),
    };
  }

  // --- Appairage et synchronisation initiale (REQ-SYN-008) ---

  /**
   * Exécute **un pas** de synchronisation initiale : récupère une page du delta depuis le curseur
   * persisté (localStorage), le met à jour, et renvoie les changements de la page + s'il en reste. Le
   * curseur survivant à un rechargement, appeler cette méthode après un `page.reload()` **reprend** au
   * dernier lot appliqué, sans repartir de zéro.
   */
  async initialSyncStep(pageSize: number): Promise<{ ids: string[]; hasMore: boolean }> {
    return this.page.evaluate(async (page) => {
      const KEY = "wallos.e2e.initialSync";
      const cursor = localStorage.getItem(KEY);
      const q = cursor
        ? `?limit=${page}&cursor=${encodeURIComponent(cursor)}`
        : `?limit=${page}`;
      const res = await fetch(`/api/v1/sync/changes${q}`);
      const body = (await res.json()) as {
        changes: { entity_type: string; id: string }[];
        next_cursor: string;
        has_more: boolean;
      };
      localStorage.setItem(KEY, body.next_cursor);
      return {
        ids: body.changes.map((c) => `${c.entity_type}:${c.id}`),
        hasMore: body.has_more,
      };
    }, pageSize);
  }

  // --- Fonctionnement hors ligne (REQ-SYN-007) ---

  /** État affiché par l'indicateur de synchronisation (`synced` / `offline` / `pending`). */
  async syncStatus(): Promise<string> {
    return (await this.page.getByTestId("sync-status").getAttribute("data-status")) ?? "";
  }

  // --- Résolution de conflit (REQ-SYN-005) ---

  /** Motifs des entrées du journal des conflits, de la plus récente à la plus ancienne. */
  async conflictReasons(): Promise<string[]> {
    return this.page.evaluate(async () => {
      const res = await fetch("/api/v1/sync/conflicts");
      const body = (await res.json()) as { conflicts: { reason: string }[] };
      return body.conflicts.map((c) => c.reason);
    });
  }

  // --- Langue (REQ-I18N-001) ---

  async setLanguage(code: string): Promise<void> {
    await this.page.getByTestId("language-select").selectOption(code);
    await this.page
      .getByTestId("language-current")
      .filter({ hasText: code })
      .waitFor({ state: "visible", timeout: 5000 });
  }

  async readLanguage(): Promise<string> {
    await this.page.getByTestId("language-current").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("language-current").textContent()) ?? "";
  }

  // --- Devise de référence (REQ-CUR-001) ---

  async setReferenceCurrency(code: string): Promise<void> {
    await this.page.getByTestId("reference-currency-input").fill(code);
    await this.page.getByTestId("reference-currency-save").click();
    // Attend que l'affichage courant reflète la nouvelle devise.
    await this.page
      .getByTestId("reference-currency-current")
      .filter({ hasText: code })
      .waitFor({ state: "visible", timeout: 5000 });
  }

  async readReferenceCurrency(): Promise<string> {
    await this.page
      .getByTestId("reference-currency-current")
      .waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("reference-currency-current").textContent()) ?? "";
  }

  // --- Moyens de paiement (REQ-SUB-011) ---

  async createPaymentMethod(name: string): Promise<void> {
    await this.page.getByTestId("payment-method-new-name").fill(name);
    await this.page.getByTestId("payment-method-create").click();
  }

  async paymentMethodVisible(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("payment-method-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async paymentMethodAbsent(name: string): Promise<boolean> {
    await this.page
      .getByTestId("payment-methods-list")
      .waitFor({ state: "visible", timeout: 5000 });
    return (
      (await this.page.getByTestId("payment-method-name").filter({ hasText: name }).count()) === 0
    );
  }

  // --- Agrégation multi-devises en mode dégradé (REQ-CUR-004) ---

  async computeAggregate(target: string, amounts: MoneyInput[]): Promise<void> {
    await this.page.getByTestId("exchange-target").fill(target);
    for (let i = 0; i < amounts.length; i++) {
      if (i > 0) {
        await this.page.getByTestId("exchange-add").click();
      }
      await this.page.getByTestId(`exchange-amount-${i}`).fill(amounts[i].amount);
      await this.page.getByTestId(`exchange-currency-${i}`).fill(amounts[i].currency);
    }
    await this.page.getByTestId("exchange-submit").click();
  }

  async aggregateIncompleteVisible(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("exchange-incomplete")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async readAggregateTotal(): Promise<string> {
    await this.page
      .getByTestId("exchange-total")
      .waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("exchange-total").textContent()) ?? "";
  }

  // --- Import / export des données (REQ-SUB-016) ---

  async exportData(): Promise<void> {
    await this.page.getByTestId("export-button").click();
  }

  async exportedBundle(): Promise<string> {
    const output = this.page.getByTestId("export-output");
    try {
      await output.waitFor({ state: "visible", timeout: 5000 });
      return await output.inputValue();
    } catch {
      return "";
    }
  }

  async importData(bundle: string): Promise<void> {
    await this.page.getByTestId("import-input").fill(bundle);
    await this.page.getByTestId("import-button").click();
  }

  async importRejectedCount(): Promise<number> {
    return this.page.getByTestId("import-rejected").count();
  }

  async importCreatedShown(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("import-created")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }
}
