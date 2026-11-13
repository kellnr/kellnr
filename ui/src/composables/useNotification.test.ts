import { test } from "node:test";
import assert from "node:assert/strict";
import { useNotification } from "./useNotification.ts";

test("all callers share one snackbar", () => {
  const first = useNotification();
  const second = useNotification();

  first.showError("Failed to revoke token");

  assert.equal(second.snackbar.show, true);
  assert.equal(second.snackbar.message, "Failed to revoke token");
  assert.equal(second.snackbar.color, "error");

  second.close();
  assert.equal(first.snackbar.show, false);
});

test("a new message replaces the current one", () => {
  const notification = useNotification();

  notification.showSuccess("Copied to clipboard");
  notification.showWarning("Session expires soon", 8000);

  assert.equal(notification.snackbar.message, "Session expires soon");
  assert.equal(notification.snackbar.color, "warning");
  assert.equal(notification.snackbar.timeout, 8000);
});
