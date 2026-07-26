import { zodResolver } from "@hookform/resolvers/zod";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { z } from "zod";

import { api, type CreateAccountRequest } from "../api/client";

/**
 * Schéma de validation du formulaire d'inscription. Les messages sont des **clés i18n**,
 * résolues à l'affichage — aucune chaîne littérale (REQ-I18N-002). La longueur minimale de
 * mot de passe (12) est identique à la validation serveur (REQ-AUT-001 / REQ-AUT-003).
 */
const signupSchema = z.object({
  email: z.string().email("signup.validation.emailInvalid"),
  password: z.string().min(12, "signup.validation.passwordTooShort"),
});

type SignupValues = z.infer<typeof signupSchema>;

// Garantit à la compilation que le formulaire produit exactement le corps attendu par le contrat.
const _typeCheck: (v: SignupValues) => CreateAccountRequest = (v) => v;
void _typeCheck;

type SubmissionState = "idle" | "success" | "error";

/**
 * Formulaire de création de compte.
 *
 * @implements REQ-AUT-001
 */
export function SignupForm() {
  const { t } = useTranslation();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<SignupValues>({ resolver: zodResolver(signupSchema) });
  const [state, setState] = useState<SubmissionState>("idle");

  async function onSubmit(values: SignupValues) {
    const { response } = await api.POST("/accounts", { body: values });
    setState(response.ok ? "success" : "error");
  }

  return (
    <form onSubmit={handleSubmit(onSubmit)} data-testid="signup-form" noValidate>
      <label htmlFor="signup-email">{t("signup.email")}</label>
      <input
        id="signup-email"
        type="email"
        data-testid="signup-email"
        aria-invalid={errors.email ? "true" : "false"}
        {...register("email")}
      />
      {errors.email?.message && (
        <p data-testid="signup-email-error" role="alert">
          {t(errors.email.message)}
        </p>
      )}

      <label htmlFor="signup-password">{t("signup.password")}</label>
      <input
        id="signup-password"
        type="password"
        data-testid="signup-password"
        aria-invalid={errors.password ? "true" : "false"}
        {...register("password")}
      />
      {errors.password?.message && (
        <p data-testid="signup-password-error" role="alert">
          {t(errors.password.message)}
        </p>
      )}

      <button type="submit" data-testid="signup-submit" disabled={isSubmitting}>
        {t("signup.submit")}
      </button>

      {state === "success" && (
        <p data-testid="signup-success" role="status">
          {t("signup.success")}
        </p>
      )}
      {state === "error" && (
        <p data-testid="signup-error" role="alert">
          {t("signup.error")}
        </p>
      )}
    </form>
  );
}
