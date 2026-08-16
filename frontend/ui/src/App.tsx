import { I18nextProvider } from "react-i18next";

import { configureApi } from "./api/client";
import { ChangePasswordForm } from "./components/ChangePasswordForm";
import { ConvertedTotalCard } from "./components/ConvertedTotalCard";
import { CategoriesList } from "./components/CategoriesList";
import { NextDueCard } from "./components/NextDueCard";
import { UpcomingPaymentsCard } from "./components/UpcomingPaymentsCard";
import { CostEvolutionCard } from "./components/CostEvolutionCard";
import { RepartitionCard } from "./components/RepartitionCard";
import { RemindersCard } from "./components/RemindersCard";
import { NotificationChannelsCard } from "./components/NotificationChannelsCard";
import { ImportExportCard } from "./components/ImportExportCard";
import { SubscriptionForm } from "./components/SubscriptionForm";
import { SubscriptionsList } from "./components/SubscriptionsList";
import { PaymentMethodsList } from "./components/PaymentMethodsList";
import { PayersList } from "./components/PayersList";
import { CurrenciesList } from "./components/CurrenciesList";
import { ReferenceCurrencySetting } from "./components/ReferenceCurrencySetting";
import { LanguageSetting } from "./components/LanguageSetting";
import { DevicesList } from "./components/DevicesList";
import { LoginForm } from "./components/LoginForm";
import { SignupForm } from "./components/SignupForm";
import { SyncStatus } from "./components/SyncStatus";
import i18n from "./i18n";

/** Modalité qui monte l'application. Seul discriminant de plateforme (ADR 0057). */
export type Canal = "web" | "desktop" | "mobile";

export type AppProps = {
  /** Modalité hôte. Exposée dans le DOM, jamais testée pour choisir un comportement d'affichage. */
  canal: Canal;
  /**
   * Origine de l'API. Vide en web — l'API est servie sur la **même origine** (REQ-OPS-003) ; une
   * coquille native doit la fournir, n'ayant aucune origine implicite (REQ-CLT-005).
   */
  apiBaseUrl?: string;
};

/**
 * Racine unique de l'interface wallos-rs, montée telle quelle par les trois coquilles.
 *
 * C'est **ici**, et nulle part ailleurs, que l'application est composée : une coquille qui
 * assemblerait des écrans deviendrait une seconde application, vouée à diverger de la première
 * (ADR 0057, repris du modèle éprouvé sur `ergonomia`). La plateforme se réduit donc à deux
 * paramètres, `canal` et `apiBaseUrl` — aucune abstraction de capacités n'est posée par avance.
 *
 * @implements REQ-CLT-003
 */
export function App({ canal, apiBaseUrl = "" }: AppProps) {
  configureApi(apiBaseUrl);

  return (
    <I18nextProvider i18n={i18n}>
      <main data-canal={canal}>
        <SyncStatus />
        <SignupForm />
        <LoginForm />
        <LanguageSetting />
        <DevicesList />
        <ChangePasswordForm />
        <ConvertedTotalCard />
        <CurrenciesList />
        <ReferenceCurrencySetting />
        <CategoriesList />
        <PaymentMethodsList />
        <PayersList />
        <NextDueCard />
        <UpcomingPaymentsCard />
        <RemindersCard />
        <NotificationChannelsCard />
        <CostEvolutionCard />
        <RepartitionCard />
        <SubscriptionForm />
        <SubscriptionsList />
        <ImportExportCard />
      </main>
    </I18nextProvider>
  );
}
