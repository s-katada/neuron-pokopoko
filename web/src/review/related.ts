import type { ReviewCard } from "../types/ReviewCard";

/** 統合カードの参照先。空なら出さない。 */
export function relatedLabel(card: ReviewCard): string | null {
  if (card.ref_titles.length === 0) {
    return null;
  }
  return `関連: ${card.ref_titles.join(" / ")}`;
}
