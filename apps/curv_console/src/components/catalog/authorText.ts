/** Author column: user ids are opaque; a null author is the system seed (`system` is the localised label). */
export function authorText(createdBy: string | null | undefined, system: string): string {
  return createdBy ? createdBy.slice(0, 8) : system;
}
