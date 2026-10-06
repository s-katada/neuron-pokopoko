export function isSessionExpired(res: {
  type: string;
  status: number;
  headers: { get(name: string): string | null };
}): boolean {
  if (res.type === "opaqueredirect") {
    return true;
  }
  if (res.status >= 300 && res.status < 400) {
    return true;
  }
  if (res.status === 401 || res.status === 403) {
    return true;
  }
  const contentType = res.headers.get("content-type")?.toLowerCase() ?? "";
  return contentType.includes("text/html");
}

export async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  const response = await fetch(path, { ...init, redirect: "manual" });
  if (isSessionExpired(response)) {
    location.reload();
  }
  return response;
}
