import { expect, test } from "bun:test";
import { runInNewContext } from "node:vm";
const view = new Bun.Transpiler({ loader: "ts" })
  .transformSync(
    await Bun.file(
      new URL("../src/shell/header/view.ts", import.meta.url),
    ).text(),
  )
  .replace(/^import .*;\n/gm, "")
  .replace(/^export /gm, "");
const render = (operation: "showHeaderProfile" | "forgetHeaderProfile") => {
  const root = { dataset: { headerProfile: "guest" } };
  const document = {
    documentElement: root,
    get cookie(): string {
      throw new Error("Header must not read HttpOnly cookies");
    },
    set cookie(_value: string) {
      throw new Error("Header must not write HttpOnly cookies");
    },
  };
  runInNewContext(`${view}\n${operation}();`, { document });
  return root.dataset.headerProfile;
};
test("confirmed session updates header display without reading profile cookies", () => {
  expect(render("showHeaderProfile")).toBe("admin");
});
test("anonymous session clears header display without writing profile cookies", () => {
  expect(render("forgetHeaderProfile")).toBe("guest");
});
