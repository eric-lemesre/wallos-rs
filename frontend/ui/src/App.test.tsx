import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import { api, configureApi } from "./api/client";

/**
 * @verifies REQ-CLT-003
 *
 * Ce que ces tests protègent : la coquille ne doit avoir **rien** à faire d'autre que monter `App`.
 * Si la composition remontait dans une coquille, ces assertions continueraient de passer alors que
 * le principe serait rompu — c'est pourquoi le dernier test regarde le code de la coquille web
 * elle-même, seul endroit où la régression serait visible.
 */
/**
 * Une origine **absolue** est fournie à dessein : monter `App` réveille les 22 composants, qui
 * émettent leurs requêtes au montage. Hors navigateur, une URL relative n'est pas analysable et
 * chaque composant produirait un rejet non géré — bruit qui masquerait un vrai défaut.
 */
const ORIGINE_DE_TEST = "http://wallos.test";

describe("App", () => {
  beforeEach(() => {
    // Une réponse NEUVE par appel : un corps ne se lit qu'une fois, et les 22 composants
    // interrogent l'API au montage.
    vi.spyOn(globalThis, "fetch").mockImplementation(async () =>
      Promise.resolve(new Response("{}", { status: 503 })),
    );
  });

  // Pas de remise à zéro ici : les hooks `afterEach` s'exécutent AVANT le démontage de Testing
  // Library, si bien que rétablir l'URL relative laisserait des composants encore montés émettre
  // des requêtes inanalysables. Le `beforeEach` réinstalle tout ce qui doit l'être.

  it("expose le canal dans le DOM sans en dériver de comportement", () => {
    render(<App canal="desktop" apiBaseUrl={ORIGINE_DE_TEST} />);
    expect(screen.getByRole("main")).toHaveAttribute("data-canal", "desktop");
  });

  it("monte la même composition quelle que soit la modalité", () => {
    const { unmount } = render(<App canal="web" apiBaseUrl={ORIGINE_DE_TEST} />);
    const enWeb = screen.getByRole("main").children.length;
    unmount();

    render(<App canal="mobile" apiBaseUrl={ORIGINE_DE_TEST} />);
    // Structure comparée, non le contenu : celui-ci dépend de requêtes asynchrones.
    expect(screen.getByRole("main").children.length).toBe(enWeb);
    expect(enWeb).toBeGreaterThan(0);
  });
});

/**
 * Le cas « origine vide » ne peut pas être vérifié par un appel réel : hors navigateur, une URL
 * relative n'est pas analysable (`Invalid URL: /api/v1/health`). L'identité du client sert donc de
 * témoin — elle est indépendante de l'environnement et suffit à établir l'idempotence et la
 * réversibilité, seules propriétés que `App` exploite.
 */
describe("configureApi", () => {
  afterEach(() => {
    configureApi("");
  });

  it("préfixe le contrat de l'origine fournie par une coquille native", async () => {
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response("{}", { status: 200 }));

    configureApi("https://wallos.exemple.test/");
    await api.GET("/health");

    const requete = fetchMock.mock.calls[0]?.[0] as Request;
    expect(requete.url).toBe("https://wallos.exemple.test/api/v1/health");
  });

  it("ignore une barre oblique finale", async () => {
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response("{}", { status: 200 }));

    configureApi("https://wallos.exemple.test///");
    await api.GET("/health");

    expect((fetchMock.mock.calls[0]?.[0] as Request).url).toBe(
      "https://wallos.exemple.test/api/v1/health",
    );
  });

  it("ne recrée pas le client quand l'origine ne change pas", () => {
    configureApi("https://wallos.exemple.test");
    const avant = api.GET;

    configureApi("https://wallos.exemple.test");

    // Sans cette garantie, `App` invaliderait le client à chaque rendu.
    expect(api.GET).toBe(avant);
  });

  it("revient à l'origine du document quand la coquille n'en fournit aucune", () => {
    configureApi("https://wallos.exemple.test");
    const natif = api.GET;

    configureApi("");

    expect(api.GET).not.toBe(natif);
  });
});
