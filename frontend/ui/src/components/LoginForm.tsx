import { zodResolver } from "@hookform/resolvers/zod";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { z } from "zod";

import { api } from "../api/client";

/**
 * Schéma du formulaire de connexion. Messages en clés i18n (REQ-I18N-002). Aucune règle de
 * politique ici : la connexion ne juge pas la force du mot de passe (REQ-AUT-002).
 */
const loginSchema = z.object({
  email: z.string().email("login.validation.emailInvalid"),
  password: z.string().min(1, "login.validation.passwordRequired"),
});

type LoginValues = z.infer<typeof loginSchema>;

/**
 * Formulaire d'authentification. Sur succès, ouvre la session (cookie) puis récupère le compte
 * courant (`GET /me`) pour prouver l'accès aux données.
 *
 * @implements REQ-AUT-002
 */
export function LoginForm() {
  const { t } = useTranslation();
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<LoginValues>({ resolver: zodResolver(loginSchema) });
  const [currentUser, setCurrentUser] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  async function onSubmit(values: LoginValues) {
    setFailed(false);
    const { response } = await api.POST("/sessions", { body: values });
    if (!response.ok) {
      setFailed(true);
      return;
    }
    const me = await api.GET("/me");
    if (me.data) {
      setCurrentUser(me.data.email);
    }
  }

  return (
    <form onSubmit={handleSubmit(onSubmit)} data-testid="login-form" noValidate>
      <label htmlFor="login-email">{t("login.email")}</label>
      <input
        id="login-email"
        type="email"
        data-testid="login-email"
        aria-invalid={errors.email ? "true" : "false"}
        {...register("email")}
      />
      {errors.email?.message && (
        <p data-testid="login-email-error" role="alert">
          {t(errors.email.message)}
        </p>
      )}

      <label htmlFor="login-password">{t("login.password")}</label>
      <input
        id="login-password"
        type="password"
        data-testid="login-password"
        {...register("password")}
      />

      <button type="submit" data-testid="login-submit" disabled={isSubmitting}>
        {t("login.submit")}
      </button>

      {failed && (
        <p data-testid="login-error" role="alert">
          {t("login.error")}
        </p>
      )}
      {currentUser && (
        <p data-testid="login-current-user" role="status">
          {t("login.loggedInAs", { email: currentUser })}
        </p>
      )}
    </form>
  );
}
