import { zodResolver } from "@hookform/resolvers/zod";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { z } from "zod";

import { api } from "../api/client";
import type { components } from "../api/client";
import { MIN_PASSWORD_LENGTH, isCompromised } from "../auth/passwordPolicy";

/**
 * Schéma du changement de mot de passe. Messages = clés i18n (REQ-I18N-002). La politique du
 * nouveau mot de passe (longueur ≥ 12, non compromis) est identique à la validation serveur
 * (REQ-AUT-003) ; le serveur reste l'autorité.
 */
const changePasswordSchema = z.object({
  current_password: z.string().min(1, "changePassword.validation.currentRequired"),
  new_password: z
    .string()
    .min(MIN_PASSWORD_LENGTH, "changePassword.validation.newTooShort")
    .refine((pw) => !isCompromised(pw), "changePassword.validation.newCompromised"),
});

type ChangePasswordValues = z.infer<typeof changePasswordSchema>;

// Garantit à la compilation que le formulaire produit exactement le corps attendu par le contrat.
const _typeCheck: (v: ChangePasswordValues) => components["schemas"]["ChangePasswordRequest"] = (
  v,
) => v;
void _typeCheck;

type State = "idle" | "success" | "wrongCurrent" | "error";

/**
 * Formulaire de changement de mot de passe. Sur succès, le serveur invalide toutes les autres
 * sessions et tous les jetons d'appareil sauf la session courante (REQ-AUT-007).
 *
 * @implements REQ-AUT-007
 */
export function ChangePasswordForm() {
  const { t } = useTranslation();
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<ChangePasswordValues>({ resolver: zodResolver(changePasswordSchema) });
  const [state, setState] = useState<State>("idle");

  async function onSubmit(values: ChangePasswordValues) {
    const { response } = await api.PUT("/password", { body: values });
    if (response.ok) {
      setState("success");
      reset();
    } else if (response.status === 403) {
      setState("wrongCurrent");
    } else {
      setState("error");
    }
  }

  return (
    <form onSubmit={handleSubmit(onSubmit)} data-testid="change-password-form" noValidate>
      <label htmlFor="change-password-current">{t("changePassword.current")}</label>
      <input
        id="change-password-current"
        type="password"
        data-testid="change-password-current"
        {...register("current_password")}
      />
      {errors.current_password?.message && (
        <p data-testid="change-password-current-error" role="alert">
          {t(errors.current_password.message)}
        </p>
      )}

      <label htmlFor="change-password-new">{t("changePassword.new")}</label>
      <input
        id="change-password-new"
        type="password"
        data-testid="change-password-new"
        {...register("new_password")}
      />
      {errors.new_password?.message && (
        <p data-testid="change-password-new-error" role="alert">
          {t(errors.new_password.message)}
        </p>
      )}

      <button type="submit" data-testid="change-password-submit" disabled={isSubmitting}>
        {t("changePassword.submit")}
      </button>

      {state === "success" && (
        <p data-testid="change-password-success" role="status">
          {t("changePassword.success")}
        </p>
      )}
      {state === "wrongCurrent" && (
        <p data-testid="change-password-wrong-current" role="alert">
          {t("changePassword.wrongCurrent")}
        </p>
      )}
      {state === "error" && (
        <p data-testid="change-password-error" role="alert">
          {t("changePassword.error")}
        </p>
      )}
    </form>
  );
}
