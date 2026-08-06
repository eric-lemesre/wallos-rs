import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { I18nextProvider } from "react-i18next";

import { ChangePasswordForm } from "../../../ui/src/components/ChangePasswordForm";
import { ConvertedTotalCard } from "../../../ui/src/components/ConvertedTotalCard";
import { CategoriesList } from "../../../ui/src/components/CategoriesList";
import { NextDueCard } from "../../../ui/src/components/NextDueCard";
import { UpcomingPaymentsCard } from "../../../ui/src/components/UpcomingPaymentsCard";
import { CostEvolutionCard } from "../../../ui/src/components/CostEvolutionCard";
import { RepartitionCard } from "../../../ui/src/components/RepartitionCard";
import { ImportExportCard } from "../../../ui/src/components/ImportExportCard";
import { SubscriptionForm } from "../../../ui/src/components/SubscriptionForm";
import { SubscriptionsList } from "../../../ui/src/components/SubscriptionsList";
import { PaymentMethodsList } from "../../../ui/src/components/PaymentMethodsList";
import { PayersList } from "../../../ui/src/components/PayersList";
import { CurrenciesList } from "../../../ui/src/components/CurrenciesList";
import { ReferenceCurrencySetting } from "../../../ui/src/components/ReferenceCurrencySetting";
import { LanguageSetting } from "../../../ui/src/components/LanguageSetting";
import { DevicesList } from "../../../ui/src/components/DevicesList";
import { LoginForm } from "../../../ui/src/components/LoginForm";
import { SignupForm } from "../../../ui/src/components/SignupForm";
import { SyncStatus } from "../../../ui/src/components/SyncStatus";
import i18n from "../../../ui/src/i18n";

// Coquille web : mince par conception (AGENTS.md §7). Elle se contente de monter l'UI partagée.
// Montage minimal signup + login (routage TanStack différé jusqu'à une exigence de navigation).
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <I18nextProvider i18n={i18n}>
        <main>
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
          <CostEvolutionCard />
          <RepartitionCard />
          <SubscriptionForm />
          <SubscriptionsList />
          <ImportExportCard />
        </main>
      </I18nextProvider>
    </StrictMode>,
  );
}
