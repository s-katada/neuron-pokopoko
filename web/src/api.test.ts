import { describe, expect, test } from "vitest";
import { isSessionExpired } from "./api";

function probe(status: number, contentType?: string, type = "basic") {
  const headers = new Headers();
  if (contentType !== undefined) {
    headers.set("content-type", contentType);
  }
  return { type, status, headers };
}

describe("isSessionExpired", () => {
  test("treats an opaque redirect as expired", () => {
    expect(isSessionExpired(probe(0, undefined, "opaqueredirect"))).toBe(true);
  });

  test("treats a redirect status as expired", () => {
    expect(isSessionExpired(probe(302))).toBe(true);
  });

  test("treats 401 and 403 as expired", () => {
    expect(isSessionExpired(probe(401))).toBe(true);
    expect(isSessionExpired(probe(403))).toBe(true);
  });

  test("treats an HTML body as expired", () => {
    expect(isSessionExpired(probe(200, "text/html; charset=utf-8"))).toBe(true);
  });

  test("leaves JSON success and 404 alone", () => {
    expect(isSessionExpired(probe(200, "application/json"))).toBe(false);
    expect(isSessionExpired(probe(404, "application/json"))).toBe(false);
  });
});
