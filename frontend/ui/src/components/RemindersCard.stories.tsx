import type { Meta, StoryObj } from "@storybook/react-vite";

import { decorateurEcran } from "../storybook/decorateurEcran";
import {
  RAPPELS,
  handlerEnPanne,
  handlerReminderSetting,
  handlerReminders,
} from "../storybook/mockApi";
import { RemindersCard } from "./RemindersCard";

/**
 * Écran RÉEL rendu sur une API simulée (ADR 0058) — le même code que la coquille web, sans
 * duplication. Chaque état n'est qu'une réponse différente du même gestionnaire, ce qui rend les
 * trois archétypes — données, vide, erreur — fidèles plutôt que reconstitués.
 *
 * Patron de référence : les autres domaines se branchent sur ce modèle (un fichier de fixtures, une
 * story par état).
 */
const meta = {
  title: "Écrans/Rappels",
  component: RemindersCard,
  parameters: { layout: "fullscreen" },
  decorators: [decorateurEcran],
} satisfies Meta<typeof RemindersCard>;

export default meta;

type Story = StoryObj<typeof meta>;

/** Cas nominal : des échéances entrent dans la fenêtre de rappel (REQ-NOT-001). */
export const AvecRappels: Story = {
  parameters: {
    msw: { handlers: [handlerReminderSetting(7), handlerReminders(RAPPELS)] },
  },
};

/** Rien à rappeler — l'état le plus fréquent au quotidien, et le plus souvent négligé. */
export const Vide: Story = {
  parameters: {
    msw: { handlers: [handlerReminderSetting(7), handlerReminders([])] },
  },
};

/** Service indisponible : l'écran doit le dire, pas rester silencieusement vide. */
export const EnPanne: Story = {
  parameters: {
    msw: { handlers: [handlerEnPanne("/settings/reminder"), handlerEnPanne("/reminders")] },
  },
};

/** Un délai de rappel long : vérifie que le libellé s'accorde (pluriel géré par l'i18n). */
export const DelaiLong: Story = {
  parameters: {
    msw: { handlers: [handlerReminderSetting(30), handlerReminders(RAPPELS.slice(0, 1))] },
  },
};
