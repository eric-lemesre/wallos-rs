import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SubscriptionLogo } from "./SubscriptionLogo";

/** @implements REQ-SUB-015 */

describe("SubscriptionLogo", () => {
  it("affiche un substitut déterministe quand aucun logo n'est fourni", () => {
    render(<SubscriptionLogo name="Netflix" />);
    const substitute = screen.getByTestId("subscription-logo-substitute");
    expect(substitute).toHaveTextContent("N");
    // Aucune image (donc aucune requête réseau vers un logo distant).
    expect(screen.queryByTestId("subscription-logo-image")).toBeNull();
  });

  it("affiche le logo fourni quand il existe", () => {
    render(<SubscriptionLogo name="Netflix" logo="netflix.png" />);
    expect(screen.getByTestId("subscription-logo-image")).toHaveAttribute("src", "netflix.png");
    expect(screen.queryByTestId("subscription-logo-substitute")).toBeNull();
  });
});
