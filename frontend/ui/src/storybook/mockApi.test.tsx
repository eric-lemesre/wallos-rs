import { render, screen } from "@testing-library/react";
import { setupServer } from "msw/node";
import { I18nextProvider } from "react-i18next";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";

import { RemindersCard } from "../components/RemindersCard";
import { configureApi } from "../api/client";
import i18n from "../i18n";
import {
  RAPPELS,
  handlerEnPanne,
  handlerReminderSetting,
  handlerReminders,
} from "./mockApi";

/**
 * @verifies REQ-CLT-008
 *
 * Garde-fou de l'infrastructure de simulation (ADR 0058). Les stories ne sont vérifiées par aucune
 * porte : si un gestionnaire cessait de correspondre aux URL du client — un préfixe qui bouge, un
 * chemin renommé —, l'atelier n'afficherait plus que des états d'erreur, et rien ne le signalerait.
 *
 * Ce test rejoue les MÊMES gestionnaires sous `msw/node` et vérifie que les trois archétypes se
 * rendent réellement. Il protège donc l'atelier de la régression silencieuse, pas les composants,
 * qui ont leurs propres tests.
 */

/** Hors navigateur, une URL relative n'est pas analysable : il faut une origine explicite. */
const ORIGINE = "http://wallos.test";

const serveur = setupServer();

beforeAll(() => {
  serveur.listen({ onUnhandledRequest: "error" });
  configureApi(ORIGINE);
});

afterEach(() => {
  serveur.resetHandlers();
});

afterAll(() => {
  serveur.close();
  configureApi("");
});

function rendre() {
  return render(
    <I18nextProvider i18n={i18n}>
      <RemindersCard />
    </I18nextProvider>,
  );
}

describe("simulation d'API de l'atelier", () => {
  it("rend l'état « avec données » comme la story homonyme", async () => {
    serveur.use(handlerReminderSetting(7), handlerReminders(RAPPELS));
    rendre();

    expect(await screen.findAllByTestId("reminder-item")).toHaveLength(RAPPELS.length);
    expect(screen.getByTestId("reminders-lead-input")).toHaveValue("7");
  });

  it("rend l'état « vide » plutôt qu'un écran muet", async () => {
    serveur.use(handlerReminderSetting(7), handlerReminders([]));
    rendre();

    expect(await screen.findByTestId("reminders-empty")).toBeInTheDocument();
  });

  it("rend l'état « en panne » avec un message, pas une liste vide trompeuse", async () => {
    serveur.use(handlerEnPanne("/settings/reminder"), handlerEnPanne("/reminders"));
    rendre();

    expect(await screen.findByTestId("reminders-error")).toBeInTheDocument();
  });
});
