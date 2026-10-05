const EMPTY = "—";

export function formatPercent(retention: number | null): string {
  if (retention === null) {
    return EMPTY;
  }
  return `${Math.round(retention * 100)}%`;
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
