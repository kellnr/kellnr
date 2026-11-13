export type ValidationRule = (value: string | null) => true | string;

export function required(fieldName: string): ValidationRule {
  return (value) => !!value || `${fieldName} is required`;
}

/**
 * Validation mode of the login form.
 *
 * Errors only show once the user submits, so tabbing past an empty field
 * does not flag it. After a failed submit, fields validate while typing,
 * so the error disappears as soon as it is fixed.
 */
export function loginValidateOn(submitAttempted: boolean): "submit" | "input" {
  return submitAttempted ? "input" : "submit";
}
