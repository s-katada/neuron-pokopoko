export function levelLabel(level: string): string {
  switch (level) {
    case "beginner":
      return "初級";
    case "intermediate":
      return "中級";
    case "advanced":
      return "上級";
    case "integration":
      return "統合";
    default:
      return level;
  }
}

export function levelBadgeClass(level: string): string {
  switch (level) {
    case "beginner":
      return "bg-sky-100 text-sky-800";
    case "intermediate":
      return "bg-amber-100 text-amber-800";
    case "advanced":
      return "bg-emerald-100 text-emerald-800";
    case "integration":
      return "bg-violet-100 text-violet-800";
    default:
      return "bg-neutral-100 text-neutral-600";
  }
}
