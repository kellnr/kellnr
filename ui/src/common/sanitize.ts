// DOMPurify options for rendering crate READMEs. On top of the defaults,
// tags and attributes that let a README restyle or overlay the page or build
// fake login forms are removed. Code highlighting uses classes only, so
// inline style attributes are not needed.
export const README_SANITIZE_CONFIG = {
  FORBID_TAGS: ["style", "form", "input", "button", "textarea", "select"],
  FORBID_ATTR: ["style"],
};
