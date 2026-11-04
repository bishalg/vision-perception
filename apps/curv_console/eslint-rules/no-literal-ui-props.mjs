/**
 * I18N-9: a string literal (or an expression-free template literal) containing a letter in a user-visible JSX
 * attribute, or in the `title`/`description` of `metadata` in `page.tsx`/`layout.tsx`, must come from the message
 * bundle instead. `className`, `href`, `type`, `id` and the like are not in the list.
 */
const UI_PROPS = new Set([
  "aria-label",
  "aria-description",
  "aria-placeholder",
  "aria-roledescription",
  "aria-valuetext",
  "title",
  "placeholder",
  "alt",
  "label",
]);
const METADATA_KEYS = new Set(["title", "description", "default", "template"]);
const HAS_LETTER = /\p{L}/u;

/** The text of a literal or an expression-free template literal, else null. */
function staticText(node) {
  if (!node) return null;
  if (node.type === "Literal" && typeof node.value === "string") return node.value;
  if (node.type === "TemplateLiteral" && node.expressions.length === 0) {
    return node.quasis.map((q) => q.value.cooked ?? "").join("");
  }
  return null;
}

const rule = {
  meta: {
    type: "problem",
    docs: { description: "Disallow literal user-visible text in JSX attributes and page metadata (I18N-9)." },
    messages: {
      attr: "Literal text in `{{name}}`: take it from the message bundle (`t.…`).",
      metadata: "Literal text in metadata `{{name}}`: take it from the message bundle in `generateMetadata`.",
    },
    schema: [],
  },
  create(context) {
    const file = context.filename ?? context.getFilename();
    const isRouteFile = /(?:^|[\\/])(?:page|layout)\.tsx$/.test(file);

    function checkMetadata(node) {
      if (node.type !== "ObjectExpression") return;
      for (const prop of node.properties) {
        if (prop.type !== "Property" || prop.computed) continue;
        const key = prop.key.type === "Identifier" ? prop.key.name : prop.key.value;
        if (!METADATA_KEYS.has(key)) continue;
        const text = staticText(prop.value);
        if (text !== null) {
          // `%s` is Next's title-template slot, not text.
          if (HAS_LETTER.test(text.replace("%s", ""))) {
            context.report({ node: prop.value, messageId: "metadata", data: { name: String(key) } });
          }
        } else if (prop.value.type === "ObjectExpression") checkMetadata(prop.value);
      }
    }

    return {
      JSXAttribute(node) {
        if (node.name.type !== "JSXIdentifier" || !UI_PROPS.has(node.name.name)) return;
        const value = node.value;
        if (!value) return;
        const expr = value.type === "JSXExpressionContainer" ? value.expression : value;
        const text = staticText(expr);
        if (text !== null && HAS_LETTER.test(text)) {
          context.report({ node: value, messageId: "attr", data: { name: node.name.name } });
        }
      },
      VariableDeclarator(node) {
        if (!isRouteFile || node.id.type !== "Identifier" || node.id.name !== "metadata" || !node.init) return;
        checkMetadata(node.init);
      },
    };
  },
};

const plugin = { rules: { "no-literal-ui-props": rule } };
export default plugin;
