/**
 * Surface publique du paquet d'interface partagé.
 *
 * Une coquille n'importe **que** ceci (ADR 0057) : elle monte `App` et ne connaît rien de
 * l'intérieur du paquet. Tout ajout ici est une décision — la surface est ce qui devra rester
 * stable pour les trois modalités.
 */
export { App } from "./App";
export type { AppProps, Canal } from "./App";
