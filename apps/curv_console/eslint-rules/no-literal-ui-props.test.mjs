import { RuleTester } from "eslint";
import { describe, it } from "vitest";
import plugin from "./no-literal-ui-props.mjs";

RuleTester.describe = describe;
RuleTester.it = it;
RuleTester.itOnly = it.only;

const tester = new RuleTester({
  languageOptions: { ecmaVersion: 2022, sourceType: "module", parserOptions: { ecmaFeatures: { jsx: true } } },
});

tester.run("no-literal-ui-props", plugin.rules["no-literal-ui-props"], {
  valid: [
    { code: "const a = <button aria-label={t.close}>x</button>;" },
    { code: 'const a = <div className="a b" href="/x" type="button" />;' },
    { code: 'const a = <img alt="" />;' },
    { code: 'const a = <input placeholder="123" />;' },
    { code: "const a = <input title={`${n}`} />;" },
    { code: 'export const metadata = { title: "Anything" };', filename: "src/lib/other.tsx" },
    { code: "export const metadata = { title: t.title };", filename: "src/app/page.tsx" },
  ],
  invalid: [
    { code: 'const a = <button aria-label="Close" />;', errors: [{ messageId: "attr" }] },
    { code: "const a = <button title={`Hi`} />;", errors: [{ messageId: "attr" }] },
    { code: 'const a = <input placeholder={"Search"} />;', errors: [{ messageId: "attr" }] },
    { code: 'const a = <img alt="Logo" />;', errors: [{ messageId: "attr" }] },
    { code: 'const a = <Select label="Yes" />;', errors: [{ messageId: "attr" }] },
    { code: 'const a = <p aria-valuetext="नमस्ते" />;', errors: [{ messageId: "attr" }] },
    {
      code: 'export const metadata = { title: { default: "Curv", template: "%s | Curv" }, description: "Hello" };',
      filename: "src/app/layout.tsx",
      errors: [{ messageId: "metadata" }, { messageId: "metadata" }, { messageId: "metadata" }],
    },
  ],
});
