import { describe, expect, it } from "vitest";
import { levelLabel } from "./level";

describe("levelLabel", () => {
  it("beginner は初級", () => {
    expect(levelLabel("beginner")).toBe("初級");
  });

  it("intermediate は中級", () => {
    expect(levelLabel("intermediate")).toBe("中級");
  });

  it("advanced は上級", () => {
    expect(levelLabel("advanced")).toBe("上級");
  });

  it("integration は統合", () => {
    expect(levelLabel("integration")).toBe("統合");
  });

  it("未知の段はそのまま返す", () => {
    expect(levelLabel("expert")).toBe("expert");
  });
});
