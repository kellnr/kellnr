import { test } from "node:test";
import assert from "node:assert/strict";
import { canModifyCrate, roleChanges, roleFlags, roleOf } from "./userRole.ts";

test("each role maps to one combination of the API flags", () => {
  assert.deepEqual(roleFlags("user"), { is_admin: false, is_read_only: false });
  assert.deepEqual(roleFlags("read-only"), { is_admin: false, is_read_only: true });
  assert.deepEqual(roleFlags("admin"), { is_admin: true, is_read_only: false });
});

test("the role of a user follows the API flags", () => {
  assert.equal(roleOf({ is_admin: false, is_read_only: false }), "user");
  assert.equal(roleOf({ is_admin: false, is_read_only: true }), "read-only");
  assert.equal(roleOf({ is_admin: true, is_read_only: false }), "admin");
});

test("admin rights win over the read-only flag, as in the backend", () => {
  assert.equal(roleOf({ is_admin: true, is_read_only: true }), "admin");
});

test("no change is needed for the current role", () => {
  assert.deepEqual(roleChanges({ is_admin: false, is_read_only: true }, "read-only"), []);
});

test("promoting a read-only user also clears the read-only flag", () => {
  assert.deepEqual(roleChanges({ is_admin: false, is_read_only: true }, "admin"), [
    { flag: "admin", state: true },
    { flag: "readOnly", state: false },
  ]);
});

test("demoting an admin to read-only changes both flags", () => {
  assert.deepEqual(roleChanges({ is_admin: true, is_read_only: false }, "read-only"), [
    { flag: "admin", state: false },
    { flag: "readOnly", state: true },
  ]);
});

test("demoting an admin with a stale read-only flag makes a plain user", () => {
  assert.deepEqual(roleChanges({ is_admin: true, is_read_only: true }, "user"), [
    { flag: "admin", state: false },
    { flag: "readOnly", state: false },
  ]);
});

test("only owners and admins with write access may change a crate", () => {
  const user = { is_admin: false, is_read_only: false };
  const readOnly = { is_admin: false, is_read_only: true };
  const admin = { is_admin: true, is_read_only: false };
  const readOnlyAdmin = { is_admin: true, is_read_only: true };

  assert.equal(canModifyCrate(user, true), true);
  assert.equal(canModifyCrate(user, false), false);
  assert.equal(canModifyCrate(readOnly, true), false);
  assert.equal(canModifyCrate(readOnly, false), false);
  // Admins are exempt from the read-only restriction, as in the backend
  assert.equal(canModifyCrate(admin, false), true);
  assert.equal(canModifyCrate(readOnlyAdmin, false), true);
});
