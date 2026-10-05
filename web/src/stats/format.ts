const EMPTY = "—";

export function formatPercent(retention: number | null): string {
  if (retention === null) {
    return EMPTY;
  }
  return `${Math.round(retention * 100)}%`;
}

export function formatDay(dayStart: number): string {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: "Asia/Tokyo",
    month: "numeric",
    day: "numeric",
  }).formatToParts(new Date(dayStart * 1000));
  const value = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((part) => part.type === type)?.value ?? "";
  return `${value("month")}/${value("day")}`;
}

export function barHeights(counts: number[]): number[] {
  const max = counts.reduce((highest, count) => Math.max(highest, count), 0);
  if (max === 0) {
    return counts.map(() => 0);
  }
  return counts.map((count) => (count / max) * 100);
}

export function formatJst(unix: number): string {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: "Asia/Tokyo",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).formatToParts(new Date(unix * 1000));
  const value = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((part) => part.type === type)?.value ?? "";
  return `${value("year")}/${value("month")}/${value("day")} ${value("hour")}:${value("minute")}`;
}
